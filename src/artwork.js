const fs=require('fs');const path=require('path');const crypto=require('crypto');
function validatedUrl(value){
 const url=new URL(value);
 if(url.protocol!=='https:'||url.username||url.password||url.port||url.search||url.hash)throw new Error('Invalid artwork URL');
 const paths={'cdn.discordapp.com':/^\/(app-icons|app-assets|quests)\/[0-9]+\//,'shared.akamai.steamstatic.com':/^\/store_item_assets\/steam\/apps\/[0-9]+\//};
 if(!paths[url.hostname]?.test(url.pathname))throw new Error('Unsupported artwork source');return url.href;
}
function imageType(bytes){if(bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))return 'png';if(bytes[0]===255&&bytes[1]===216)return 'jpg';if(bytes.toString('ascii',0,4)==='RIFF'&&bytes.toString('ascii',8,12)==='WEBP')return 'webp';throw new Error('Unsupported image format');}
class ArtworkCache{
 constructor(directory=path.resolve(__dirname,'../data/artwork')){this.directory=directory;this.pending=new Map();this.failures=new Map();this.active=0;this.waiters=[];}
 key(url){return crypto.createHash('sha256').update(validatedUrl(url)).digest('hex');}
 read(url){const key=this.key(url);for(const type of ['png','jpg','webp']){const file=path.join(this.directory,key+'.'+type);try{const bytes=fs.readFileSync(file);return {bytes,type:imageType(bytes)};}catch{}}return null;}
 async get(url){url=validatedUrl(url);const cached=this.read(url);if(cached)return cached;if(this.pending.has(url))return this.pending.get(url);if(Date.now()-(this.failures.get(url)||0)<30000)throw new Error('Artwork unavailable');
  const work=this.download(url).catch(e=>{this.failures.set(url,Date.now());if(this.failures.size>1000)this.failures.delete(this.failures.keys().next().value);throw e;}).finally(()=>this.pending.delete(url));this.pending.set(url,work);return work;
 }
 async download(url){if(this.active>=4)await new Promise(r=>this.waiters.push(r));else this.active++;
  try{const response=await fetch(url,{signal:AbortSignal.timeout(8000),redirect:'error'});if(!response.ok)throw new Error('Artwork HTTP '+response.status);const chunks=[];let length=0;for await(const chunk of response.body){length+=chunk.length;if(length>4*1024*1024)throw new Error('Artwork too large');chunks.push(chunk);}const bytes=Buffer.concat(chunks);const type=imageType(bytes);fs.mkdirSync(this.directory,{recursive:true});fs.writeFileSync(path.join(this.directory,this.key(url)+'.'+type),bytes);this.trim();return {bytes,type};}
  finally{const next=this.waiters.shift();if(next)next();else this.active--;}
 }
 trim(){const entries=fs.readdirSync(this.directory).filter(n=>/^[a-f0-9]{64}\.(png|jpg|webp)$/.test(n)).map(name=>{const file=path.resolve(this.directory,name);const stat=fs.statSync(file);return {file,size:stat.size,time:stat.mtimeMs};}).sort((a,b)=>a.time-b.time);let total=entries.reduce((n,e)=>n+e.size,0);for(const entry of entries){if(total<=64*1024*1024)break;if(!entry.file.startsWith(path.resolve(this.directory)+path.sep))throw new Error('Invalid cache path');fs.unlinkSync(entry.file);total-=entry.size;}}
}
module.exports=new ArtworkCache();module.exports.ArtworkCache=ArtworkCache;module.exports.validatedUrl=validatedUrl;
