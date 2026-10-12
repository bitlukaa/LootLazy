param(
 [Parameter(Mandatory=$true)][string[]]$FilePath,
 [string]$CertificateThumbprint,
 [string]$TimestampServer,
 [switch]$CheckOnly
)
$ErrorActionPreference='Stop'
$targets=@($FilePath | ForEach-Object {
 $item=Get-Item -LiteralPath $_
 if($item.PSIsContainer -or $item.Extension -ine '.exe'){throw 'Only existing executable files can be signed.'}
 $item.FullName
})
if($CheckOnly){
 foreach($target in $targets){$status=Get-AuthenticodeSignature -LiteralPath $target;[pscustomobject]@{File=$target;Status=[string]$status.Status;SigningConfigured=[bool]$CertificateThumbprint}}
 return
}
if(!$CertificateThumbprint -or $CertificateThumbprint -notmatch '^[a-fA-F0-9]{40}$'){throw 'A production code-signing certificate thumbprint is required. No files were changed.'}
if(!$TimestampServer){throw 'Use the timestamp server supplied by the selected certificate provider.'}
$url=[Uri]$TimestampServer
if(!$url.IsAbsoluteUri -or $url.Scheme -notin @('http','https')){throw 'Invalid timestamp server URL.'}
$certificate=Get-Item -LiteralPath (Join-Path 'Cert:\CurrentUser\My' $CertificateThumbprint) -ErrorAction Stop
if(!$certificate.HasPrivateKey -or $certificate.NotAfter -lt (Get-Date) -or $certificate.NotBefore -gt (Get-Date)){throw 'The certificate must be valid and have an accessible private key.'}
$usageExtension=@($certificate.Extensions | Where-Object {$_.Oid.Value -eq '2.5.29.37'})
$codeSigning=@($usageExtension | ForEach-Object {$_.EnhancedKeyUsages} | Where-Object {$_.Value -eq '1.3.6.1.5.5.7.3.3'})
if($codeSigning.Count -eq 0){throw 'The selected certificate is not a code-signing certificate.'}
if($certificate.Subject -eq $certificate.Issuer){throw 'Self-signed certificates are not accepted for production releases.'}
$chain=New-Object Security.Cryptography.X509Certificates.X509Chain
try{
 $chain.ChainPolicy.RevocationMode=[Security.Cryptography.X509Certificates.X509RevocationMode]::Online
 $chain.ChainPolicy.UrlRetrievalTimeout=[TimeSpan]::FromSeconds(10)
 if(!$chain.Build($certificate)){throw 'The certificate does not chain to a trusted authority.'}
}finally{$chain.Dispose()}
foreach($target in $targets){
 $result=Set-AuthenticodeSignature -LiteralPath $target -Certificate $certificate -HashAlgorithm SHA256 -IncludeChain NotRoot -TimestampServer $TimestampServer -ErrorAction Stop
 if($result.Status -ne 'Valid'){throw "Signature validation failed for $target"}
 if(!$result.TimeStamperCertificate){throw "The required timestamp was not applied to $target"}
 $verified=Get-AuthenticodeSignature -LiteralPath $target
 if($verified.Status -ne 'Valid' -or $verified.SignerCertificate.Thumbprint -ne $certificate.Thumbprint){throw 'Publisher verification failed after signing.'}
 [pscustomobject]@{File=$target;Status='Valid';Publisher=$verified.SignerCertificate.Subject;Timestamped=$true}
}
Write-Output 'Authenticode changes executable hashes. Recreate portable archives and detached update signatures before publication.'