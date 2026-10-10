const fs=require('fs'),path=require('path');
function configurePortableStorage(app,applicationRoot,options={}){
 const data=path.resolve(applicationRoot,'data');
 const locations={userData:path.join(data,'electron'),sessionData:path.join(data,'electron'),temp:path.join(data,'temp'),logs:path.join(data,'logs'),crashDumps:path.join(data,'crash-dumps')};
 const previous=app.getPath('userData');
 for(const directory of new Set(Object.values(locations)))fs.mkdirSync(directory,{recursive:true});
 // Read the old Questro display preferences only; never write to the previous profile.
 const oldStorage=path.join(previous,'Local Storage'),newStorage=path.join(locations.userData,'Local Storage');
 if(options.migratePreferences!==false&&path.resolve(previous)!==locations.userData&&!fs.existsSync(newStorage)&&fs.existsSync(oldStorage)){
  try{fs.cpSync(oldStorage,newStorage,{recursive:true,filter:file=>path.basename(file)!=='LOCK'});}catch{console.warn('Display preference migration skipped; portable storage remains enabled.');}
 }
 for(const [name,directory]of Object.entries(locations))app.setPath(name,directory);
 process.env.TEMP=locations.temp;process.env.TMP=locations.temp;process.env.TMPDIR=locations.temp;
 app.commandLine.appendSwitch('disk-cache-dir',path.join(locations.sessionData,'Cache'));
 return locations;
}
module.exports={configurePortableStorage};
