(() => {
 const delay=250;
 window.lootlazyReorder={attach(list,{selector,onMove,canMove=()=>true}){
  let drag=null,timer=null,suppressClick=false,frame=null;
  const rowAt=target=>target?.closest(selector);
  function clear(){clearTimeout(timer);cancelAnimationFrame(frame);timer=frame=null;list.dataset.reordering='false';if(drag){drag.row.classList.remove('reorder-dragging');drag.target?.classList.remove('reorder-target');try{list.releasePointerCapture(drag.pointer);}catch{}}drag=null;}
  function locate(){if(!drag?.active)return;const target=rowAt(document.elementFromPoint(drag.x,drag.y));drag.target?.classList.remove('reorder-target');drag.target=null;if(target&&target!==drag.row&&list.contains(target)&&canMove(target)){drag.target=target;drag.after=drag.y>target.getBoundingClientRect().top+target.offsetHeight/2;target.classList.add('reorder-target');target.dataset.dropAfter=String(drag.after);}}
  function scroll(){if(!drag?.active)return;const rect=list.getBoundingClientRect();if(drag.y<rect.top+32)list.scrollTop-=8;else if(drag.y>rect.bottom-32)list.scrollTop+=8;locate();frame=requestAnimationFrame(scroll);}
  function activate(){if(!drag||!drag.row.isConnected){clear();return;}drag.active=true;suppressClick=true;drag.row.classList.add('reorder-dragging');try{list.setPointerCapture(drag.pointer);}catch{}locate();frame=requestAnimationFrame(scroll);}
  list.addEventListener('dragstart',event=>event.preventDefault());
  list.addEventListener('pointerdown',event=>{
   if(event.button!==0||drag)return;const row=rowAt(event.target);if(!row||!list.contains(row)||!canMove(row)||event.target.closest('input,select,textarea,button:not(.account-select)'))return;
   drag={row,pointer:event.pointerId,x:event.clientX,y:event.clientY,startX:event.clientX,startY:event.clientY,active:false,target:null,after:false};list.dataset.reordering='true';timer=setTimeout(activate,delay);
   if(event.target.closest('.reorder-handle')){event.preventDefault();clearTimeout(timer);activate();}
  });
  list.addEventListener('pointermove',event=>{if(!drag||drag.pointer!==event.pointerId)return;drag.x=event.clientX;drag.y=event.clientY;if(!drag.active){if(Math.hypot(drag.x-drag.startX,drag.y-drag.startY)>6){if(event.pointerType==='touch'){clear();return;}clearTimeout(timer);activate();}return;}event.preventDefault();locate();},{passive:false});
  list.addEventListener('pointerup',event=>{if(!drag||event.pointerId!==drag.pointer)return;const current=drag;clear();if(current.active&&current.target?.isConnected)Promise.resolve(onMove(current.row,current.target,current.after)).catch(error=>toast(errorText(error),'error'));setTimeout(()=>{suppressClick=false;},0);});
  for(const type of ['pointercancel','lostpointercapture'])list.addEventListener(type,()=>{clear();setTimeout(()=>{suppressClick=false;},0);});
  list.addEventListener('click',event=>{if(suppressClick){event.preventDefault();event.stopImmediatePropagation();}},{capture:true});
  list.addEventListener('keydown',event=>{if(!event.altKey||!['ArrowUp','ArrowDown'].includes(event.key))return;const row=rowAt(event.target),rows=[...list.querySelectorAll(selector)].filter(canMove),index=rows.indexOf(row),offset=event.key==='ArrowUp'?-1:1,target=rows[index+offset];if(index<0||!target)return;event.preventDefault();Promise.resolve(onMove(row,target,offset>0)).catch(error=>toast(errorText(error),'error'));});
 }};
})();
