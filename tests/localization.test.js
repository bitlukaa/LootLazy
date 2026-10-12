const test=require('node:test');const assert=require('node:assert/strict');const fs=require('fs');const path=require('path');const vm=require('vm');
const root=path.resolve(__dirname,'..');
test('English translations and all static UI translation keys are complete',()=>{
 const source=fs.readFileSync(path.join(root,'public/js/i18n.js'),'utf8');const dictionaries=vm.runInNewContext(source+';I18N',{localStorage:{getItem:()=>null}});
 assert.deepEqual(Object.keys(dictionaries.ar).sort(),Object.keys(dictionaries.en).sort());
 for(const [key,value]of Object.entries(dictionaries.en))assert.equal(/[\u0600-\u06ff]/.test(value),false,key);
 const html=fs.readFileSync(path.join(root,'public/index.html'),'utf8');
 for(const match of html.matchAll(/data-i18n(?:-title|-aria-label|-placeholder)?="([^"]+)"/g))for(const lang of ['ar','en'])assert.ok(dictionaries[lang][match[1]],lang+': '+match[1]);
 const app=fs.readFileSync(path.join(root,'public/js/app.js'),'utf8')+fs.readFileSync(path.join(root,'public/js/rotations.js'),'utf8');
 for(const match of app.matchAll(/\bt\('([^']+)'\s*[,)]/g))for(const lang of ['ar','en'])assert.ok(Object.hasOwn(dictionaries[lang],match[1]),lang+': '+match[1]);
});
test('maintained README documents are English',()=>{
 for(const file of ['README.md'])assert.equal(/[\u0600-\u06ff]/.test(fs.readFileSync(path.join(root,file),'utf8')),false,file);
});

test('English startup keeps the numeric formatter available',()=>{const source=fs.readFileSync(path.join(root,'public/js/i18n.js'),'utf8');assert.equal(vm.runInNewContext(source+';number(24634)',{localStorage:{getItem:()=>null}}),'24,634');});
