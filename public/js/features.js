(() => {
 const key='lootlazy_used_features';let used=new Set();
 try{const saved=JSON.parse(localStorage.getItem(key)||'[]');if(Array.isArray(saved))used=new Set(saved.filter(value=>typeof value==='string'));}catch{}
 function render(){for(const node of document.querySelectorAll('[data-feature-new]')){let badge=node.querySelector('.feature-new-badge');if(!badge){badge=document.createElement('span');badge.className='feature-new-badge';badge.dataset.i18n='feature_new';node.append(badge);}badge.textContent=t('feature_new');badge.hidden=used.has(node.dataset.featureNew);}}
 window.lootlazyFeatures={markUsed(id){if(used.has(id))return;used.add(id);try{localStorage.setItem(key,JSON.stringify([...used]));}catch{}render();}};
 document.addEventListener('DOMContentLoaded',render);
})();
