const fs=require('fs');
(async()=>{
 const svgResponse=await fetch('https://raw.githubusercontent.com/lucide-icons/lucide/main/icons/gamepad-2.svg');if(!svgResponse.ok)throw new Error('Logo HTTP '+svgResponse.status);const original=await svgResponse.text();
 const licenseResponse=await fetch('https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE');if(!licenseResponse.ok)throw new Error('License download failed');fs.writeFileSync('public/assets/third-party/icons.txt',await licenseResponse.text());
 const contents=original.slice(original.indexOf('>')+1,original.lastIndexOf('</svg>'));
 const logo='<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128"><defs><linearGradient id="brand" x2="1" y2="1"><stop stop-color="#8d7cff"/><stop offset="1" stop-color="#5542be"/></linearGradient></defs><rect x="3" y="3" width="122" height="122" rx="30" fill="url(#brand)"/><g transform="translate(18 18) scale(3.83)" fill="none" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">'+contents+'</g></svg>';
 fs.writeFileSync('public/assets/questro-logo.svg',logo);
 const sharp=require(process.env.QUESTRO_SHARP||'sharp');await sharp(Buffer.from(logo)).resize(256,256).png().toFile('public/assets/questro-logo.png');
 const sizes=[256,128,64,48,32,16],images=[];for(const size of sizes)images.push(await sharp(Buffer.from(logo)).resize(size,size).png().toBuffer());
 const header=Buffer.alloc(6+sizes.length*16);header.writeUInt16LE(1,2);header.writeUInt16LE(sizes.length,4);let offset=header.length;images.forEach((bytes,i)=>{const entry=6+i*16;header[entry]=sizes[i]===256?0:sizes[i];header[entry+1]=header[entry];header.writeUInt16LE(1,entry+4);header.writeUInt16LE(32,entry+6);header.writeUInt32LE(bytes.length,entry+8);header.writeUInt32LE(offset,entry+12);offset+=bytes.length;});const ico=Buffer.concat([header,...images]);fs.writeFileSync('public/favicon.ico',ico);fs.writeFileSync('Questro.ico',ico);
 console.log('Generated unified Questro logo from Lucide gamepad-2.');
})().catch(error=>{console.error(error);process.exitCode=1});
