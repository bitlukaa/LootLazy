(() => {
 const delay=320;
 window.lootlazyReorder={attach(list,{selector,onMove,canMove=()=>true}){
  let drag=null,timer=null,suppressClick=false;
  const rowAt=target=>target?.closest(selector);
  function clear(){clearTimeout(timer);timer=null;if(drag){drag.row.classList.remove('reorder-dragging');drag.target?.classList.remove('reorder-target');try{list.releasePointerCapture(drag.pointer);}catch{}}drag=null;}
  list.addEventListener('pointerdown',event=>{
   if(event.button!==0||drag)return;const row=rowAt(event.target);if(!row||!list.contains(row)||!canMove(row)||event.target.closest('input,select,textarea,button:not(.account-select)'))return;
   drag={row,pointer:event.pointerId,x:event.clientX,y:event.clientY,active:false,target:null,after:false};
   timer=setTimeout(()=>{if(!drag||!drag.row.isConnected){clear();return;}drag.active=true;suppressClick=true;row.classList.add('reorder-dragging');list.setPointerCapture(event.pointerId);},delay);
  });
  list.addEventListener('pointermove',event=>{
   if(!drag||drag.pointer!==event.pointerId)return;
   if(!drag.active){if(Math.hypot(event.clientX-drag.x,event.clientY-drag.y)>8)clear();return;}
   event.preventDefault();const target=rowAt(document.elementFromPoint(event.clientX,event.clientY));drag.target?.classList.remove('reorder-target');drag.target=null;
   if(target&&target!==drag.row&&list.contains(target)&&canMove(target)){drag.target=target;drag.after=event.clientY>target.getBoundingClientRect().top+target.offsetHeight/2;target.classList.add('reorder-target');}
   const rect=list.getBoundingClientRect();if(event.clientY<rect.top+30)list.scrollTop-=14;else if(event.clientY>rect.bottom-30)list.scrollTop+=14;
  },{passive:false});
  list.addEventListener('pointerup',event=>{if(!drag||event.pointerId!==drag.pointer)return;const current=drag;clear();if(current.active&&current.target?.isConnected)onMove(current.row,current.target,current.after);setTimeout(()=>{suppressClick=false;},0);});
  for(const type of ['pointercancel','lostpointercapture'])list.addEventListener(type,clear);
  list.addEventListener('click',event=>{if(suppressClick){event.preventDefault();event.stopImmediatePropagation();}},{capture:true});
  list.addEventListener('keydown',event=>{if(!event.altKey||!['ArrowUp','ArrowDown'].includes(event.key))return;const row=rowAt(event.target),rows=[...list.querySelectorAll(selector)].filter(canMove),index=rows.indexOf(row),offset=event.key==='ArrowUp'?-1:1,target=rows[index+offset];if(index<0||!target)return;event.preventDefault();onMove(row,target,offset>0);});
 }};
})();
