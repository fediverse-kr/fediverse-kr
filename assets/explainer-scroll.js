// Browser geometry only. Rust owns the scene and its accessible content.
const root = document.getElementById('ex-scrolly');
if (!root) return;
window.__fedkrExplainerScroll?.dispose();
const steps = [...root.querySelectorAll('[data-scroll-step]')];
const nav = document.querySelector('.ex-chapters');
const stage = root.querySelector('.ex-stage-wrap');
const abort = new AbortController();
let raf = 0, last = -1;
const update = () => {
  raf = 0;
  if (!root.isConnected) { dispose(); return; }
  const mobile = matchMedia('(max-width: 760px)').matches;
  const navHeight = nav?.getBoundingClientRect().height || 60;
  root.style.setProperty('--ex-nav-height', `${navHeight}px`);
  const anchor = mobile ? navHeight + stage.offsetHeight + 62 : innerHeight * .5;
  let nearest = 0, distance = Infinity;
  steps.forEach((step, i) => {
    const box = step.getBoundingClientRect();
    const point = mobile ? box.top + 72 : box.top + box.height * .5;
    if (Math.abs(point-anchor) < distance) { distance = Math.abs(point-anchor); nearest = i; }
  });
  if (nearest !== last) { last = nearest; dioxus.send(nearest); }
};
const queue = () => { if (!raf) raf = requestAnimationFrame(update); };
const resize = new ResizeObserver(queue);
resize.observe(root); if(nav) resize.observe(nav);
const removal = new MutationObserver(() => { if(!root.isConnected) dispose(); });
removal.observe(document.body, {childList:true,subtree:true});
function dispose() {
  abort.abort(); resize.disconnect(); removal.disconnect(); cancelAnimationFrame(raf);
  if (window.__fedkrExplainerScroll?.root === root) delete window.__fedkrExplainerScroll;
}
window.__fedkrExplainerScroll = {
  root, dispose,
  go(index) {
    const step = steps[index]; if (!step) return;
    const mobile = matchMedia('(max-width: 760px)').matches;
    const navHeight = nav?.getBoundingClientRect().height || 60;
    const anchor = mobile ? navHeight + stage.offsetHeight + 62 : innerHeight * .5;
    const offset = mobile ? 72 : step.offsetHeight * .5;
    window.scrollTo({top:Math.max(0,scrollY+step.getBoundingClientRect().top+offset-anchor),behavior:'smooth'});
  }
};
window.addEventListener('scroll',queue,{passive:true,signal:abort.signal});
window.addEventListener('resize',queue,{passive:true,signal:abort.signal});
window.addEventListener('pagehide',dispose,{signal:abort.signal});
queue();
