'use strict';
const $=id=>document.getElementById(id);
const xyz=['x','y','z'];
const controls=['px','py','pz','rx','ry','rz'];
const changes=new Map();
let catalog=[],current=null,marker=null,mode=null,firstPick=null,serial=0;
const dot=(a,b)=>a.reduce((s,v,i)=>s+v*b[i],0);
const add=(a,b)=>a.map((v,i)=>v+b[i]);
const sub=(a,b)=>a.map((v,i)=>v-b[i]);
const mul=(v,k)=>v.map(x=>x*k);
const cross=(a,b)=>[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
function rotation(r){
 const [sx,sy,sz]=r.map(Math.sin),[cx,cy,cz]=r.map(Math.cos);
 return [[cy*cz,-cy*sz,sy],[sy*sx*cz+cx*sz,-sx*sy*sz+cx*cz,-sx*cy],[-cx*sy*cz+sx*sz,cx*sy*sz+sx*cz,cx*cy]];
}
const rowMul=(v,m)=>[0,1,2].map(i=>v.reduce((s,x,j)=>s+x*m[j][i],0));
function camera(loc,offset){
 const r=rotation(loc.rotation),basis=[mul(r[0],-1),r[1],mul(r[2],-1)];
 return {basis,origin:add(loc.position,mul(basis[2],offset))};
}
function cameraPoint(p,cam){return cam.basis.map(axis=>dot(sub(p,cam.origin),axis));}
function proposed(){
 return {position:current.locator.position.map((v,i)=>v+Number($('p'+xyz[i]).value)/1000),
 rotation:current.locator.rotation.map((v,i)=>v+Number($('r'+xyz[i]).value)*Math.PI/180)};
}
function original(){return {position:current.locator.position.slice(),rotation:current.locator.rotation.slice()};}
function changed(a,b){return [...a.position,...a.rotation].some((v,i)=>Math.abs(v-[...b.position,...b.rotation][i])>1e-10);}
function safeValues(){return [...controls,'fov','zoom'].every(id=>Number.isFinite(Number($(id).value)))&&Number($('fov').value)>0&&Number($('fov').value)<180&&Number($('zoom').value)>0;}
class View {
 constructor(id){
  this.canvas=$(id);this.overlay=$(id+'Overlay');this.gl=this.canvas.getContext('webgl',{antialias:true,preserveDrawingBuffer:true});
  if(!this.gl)throw Error('WebGL is unavailable');
  const gl=this.gl;
  const shader=(kind,code)=>{const s=gl.createShader(kind);gl.shaderSource(s,code);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw Error(gl.getShaderInfoLog(s));return s;};
  this.program=gl.createProgram();
  gl.attachShader(this.program,shader(gl.VERTEX_SHADER,`attribute vec3 p;attribute vec2 uv;uniform vec3 origin,axisX,axisY,axisZ;uniform float f,aspect;varying vec2 texcoord;void main(){vec3 d=p-origin;vec3 q=vec3(dot(d,axisX),dot(d,axisY),dot(d,axisZ));gl_Position=vec4(f*q.x/aspect,f*q.y,1.00004*q.z-0.00400008,q.z);texcoord=uv;}`));
  gl.attachShader(this.program,shader(gl.FRAGMENT_SHADER,`precision mediump float;varying vec2 texcoord;uniform sampler2D tex;uniform bool emitter;void main(){vec4 c=texture2D(tex,texcoord);if(c.a<0.5)discard;gl_FragColor=emitter?vec4(1.0,0.16,0.10,1.0):vec4(c.rgb,1.0);}`));
  gl.linkProgram(this.program);if(!gl.getProgramParameter(this.program,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(this.program));
  this.meshes=[];this.generation=0;
 }
 dispose(m){const gl=this.gl;gl.deleteBuffer(m.p);gl.deleteBuffer(m.uv);gl.deleteBuffer(m.ix);gl.deleteTexture(m.texture);}
 clear(){this.generation++;for(const m of this.meshes)this.dispose(m);this.meshes=[];}
 async load(scene){
  this.clear();const gl=this.gl,generation=this.generation;
  for(const mesh of scene.meshes){
   const buffer=(target,data)=>{const b=gl.createBuffer();gl.bindBuffer(target,b);gl.bufferData(target,data,gl.STATIC_DRAW);return b;};
   const m={source:mesh,p:buffer(gl.ARRAY_BUFFER,new Float32Array(mesh.positions.flat())),uv:buffer(gl.ARRAY_BUFFER,new Float32Array(mesh.uv.flat())),ix:buffer(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array(mesh.triangles.flat())),count:mesh.triangles.length*3};
   m.texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,m.texture);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,1,1,0,gl.RGBA,gl.UNSIGNED_BYTE,new Uint8Array([160,160,160,255]));
   // Decode before upload; immutable local scene assets have no external URLs.
   const im=new Image();im.src=mesh.texture;try{await im.decode();}catch(error){this.dispose(m);throw error;}
   if(generation!==this.generation){this.dispose(m);return;}
   gl.bindTexture(gl.TEXTURE_2D,m.texture);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,im);
   gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
   this.meshes.push(m);
  }
 }
 visible(m){return $('glass').checked||!m.source.glass;}
 draw(loc){
  const gl=this.gl,w=Math.max(320,Math.round(this.canvas.clientWidth*devicePixelRatio)),h=Math.round(w*9/16);
  this.canvas.width=this.overlay.width=w;this.canvas.height=this.overlay.height=h;
  const cam=camera(loc,current.offset),f=Number($('zoom').value)/Math.tan(Number($('fov').value)*Math.PI/360);
  gl.viewport(0,0,w,h);gl.clearColor(.235,.255,.275,1);gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT);gl.enable(gl.DEPTH_TEST);gl.disable(gl.CULL_FACE);gl.useProgram(this.program);
  const u=name=>gl.getUniformLocation(this.program,name);
  gl.uniform3fv(u('origin'),cam.origin);['axisX','axisY','axisZ'].forEach((n,i)=>gl.uniform3fv(u(n),cam.basis[i]));gl.uniform1f(u('f'),f);gl.uniform1f(u('aspect'),w/h);gl.uniform1i(u('tex'),0);
  for(const m of this.meshes){if(!this.visible(m))continue;
   for(const [name,b,size] of [['p',m.p,3],['uv',m.uv,2]]){const at=gl.getAttribLocation(this.program,name);gl.bindBuffer(gl.ARRAY_BUFFER,b);gl.enableVertexAttribArray(at);gl.vertexAttribPointer(at,size,gl.FLOAT,false,0,0);}
   gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,m.texture);gl.uniform1i(u('emitter'),m.source.emitter);gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,m.ix);gl.drawElements(gl.TRIANGLES,m.count,gl.UNSIGNED_SHORT,0);
  }
  const ctx=this.overlay.getContext('2d');ctx.clearRect(0,0,w,h);ctx.strokeStyle='#69ff91';ctx.lineWidth=1.5*devicePixelRatio;
  ctx.beginPath();ctx.moveTo(w/2-10,h/2);ctx.lineTo(w/2+10,h/2);ctx.moveTo(w/2,h/2-10);ctx.lineTo(w/2,h/2+10);ctx.stroke();
  if(marker){const q=cameraPoint(marker,cam);if(q[2]>.002){const x=w/2+h/2*f*q[0]/q[2],y=h/2-h/2*f*q[1]/q[2];ctx.strokeStyle='#ff806b';ctx.beginPath();ctx.arc(x,y,5*devicePixelRatio,0,Math.PI*2);ctx.stroke();}}
 }
 pick(event,loc){
  const rect=this.canvas.getBoundingClientRect(),f=Number($('zoom').value)/Math.tan(Number($('fov').value)*Math.PI/360),cam=camera(loc,current.offset);
  const x=(event.clientX-rect.left-rect.width/2)/(rect.height/2*f),y=-(event.clientY-rect.top-rect.height/2)/(rect.height/2*f);
  const ray=add(add(mul(cam.basis[0],x),mul(cam.basis[1],y)),cam.basis[2]);let best=null,distance=Infinity;
  for(const m of this.meshes){if(!this.visible(m)||$('surface').value!=='all'&&$('surface').value!==m.source.name)continue;
   const p=m.source.positions;
   for(const [i,j,k]of m.source.triangles){const a=p[i],e1=sub(p[j],a),e2=sub(p[k],a),h=cross(ray,e2),det=dot(e1,h);if(Math.abs(det)<1e-12)continue;
    const s=sub(cam.origin,a),u=dot(s,h)/det;if(u<0||u>1)continue;const q=cross(s,e1),v=dot(ray,q)/det;if(v<0||u+v>1)continue;
    const t=dot(e2,q)/det;if(t>.002&&t<distance){distance=t;best=add(cam.origin,mul(ray,t));}
   }
  }
  return best;
 }
}
let views;
function remember(){
 if(!current||!safeValues())return;
 const before=original(),after=proposed();
 if(changed(before,after))changes.set(current.file,{file:current.file,sha256:current.sha256,before,after,example:current.id,marker,verification:'offline candidate; requires in-game check'});
 else changes.delete(current.file);
 $('planCount').textContent=`${changes.size} model${changes.size===1?'':'s'}`;
}
function refresh(){
 if(!current||!safeValues())return;
 remember();views[0].draw(original());views[1].draw(proposed());
 const f=540/Math.tan(Number($('fov').value)*Math.PI/360);
 const describe=loc=>{if(!marker)return 'no sight point selected';const q=cameraPoint(marker,camera(loc,current.offset));return q[2]<=0?'point behind camera':`${(f*q[0]/q[2]).toFixed(2)} px right, ${(-f*q[1]/q[2]).toFixed(2)} px down`;};
 $('status').textContent=`At 1080 lines, ${Number($('fov').value).toFixed(2)}° vertical FOV (inspection zoom excluded):\nReleased: ${describe(original())}\nProposed: ${describe(proposed())}\n${current.marker_basis}\n${current.file}`;
 $('dirty').textContent=changes.has(current.file)?'unsaved candidate':'';
}
async function select(id){
 remember();const token=++serial,row=catalog.find(r=>r.id===id);current=null;marker=null;
 $('title').textContent=id;$('unsupported').textContent=row.reason||'';$('shared').textContent=(row.shared||[]).join('\n');$('status').textContent='';$('dirty').textContent='';
 for(const key of controls)$(key).disabled=row.status==='unsupported';
 if(row.status==='unsupported'){for(const v of views){v.clear();v.gl.clear(v.gl.COLOR_BUFFER_BIT|v.gl.DEPTH_BUFFER_BIT);v.overlay.getContext('2d').clearRect(0,0,v.overlay.width,v.overlay.height);}return;}
 const response=await fetch('scenes/'+encodeURIComponent(id)+'.json');if(!response.ok)throw Error(await response.text());const s=await response.json();if(token!==serial)return;
 $('surface').replaceChildren(new Option('All visible surfaces','all'),...s.meshes.map(m=>new Option(m.name,m.name)));
 $('fov').value=s.fov;for(const id of controls)$(id).value=0;
 const saved=changes.get(s.file);
 if(saved){xyz.forEach((axis,i)=>{$('p'+axis).value=(saved.after.position[i]-s.locator.position[i])*1000;$('r'+axis).value=(saved.after.rotation[i]-s.locator.rotation[i])*180/Math.PI;});}
 await Promise.all(views.map(v=>v.load(s)));if(token!==serial)return;
 current=s;marker=saved?.example===s.id?saved.marker:s.marker;mode=null;firstPick=null;refresh();
}
function filter(){const needle=$('search').value.toLowerCase().replaceAll(/[^a-z0-9]/g,'');const rows=catalog.filter(r=>r.id.replaceAll(/[^a-z0-9]/g,'').includes(needle));const selected=$('pairs').value;
 $('pairs').replaceChildren(...rows.map(r=>new Option(`${r.status==='unsupported'?'⚠ ':''}${r.id}`,r.id)));if(rows.some(r=>r.id===selected))$('pairs').value=selected;$('count').textContent=`${rows.length} / ${catalog.length} pairs`;}
function fail(error){$('unsupported').textContent=error.message||String(error);console.error(error);}
async function start(){
 views=[new View('before'),new View('after')];catalog=await (await fetch('catalog.json')).json();filter();
 $('search').oninput=filter;$('pairs').onchange=()=>select($('pairs').value).catch(fail);
 for(const id of [...controls,'zoom','fov','glass'])$(id).oninput=refresh;
 $('pick').onclick=()=>{mode='point';firstPick=null;$('status').textContent='Click a sight point on either model.';};
 $('midpoint').onclick=()=>{mode='midpoint';firstPick=null;$('status').textContent='Click two opposite points on the same sight rim.';};
 views.forEach((v,i)=>v.canvas.onclick=event=>{if(!mode||!current)return;const point=v.pick(event,i?proposed():original());if(!point){$('status').textContent='No surface hit. Select a surface and click its visible geometry.';return;}
  if(mode==='midpoint'&&!firstPick){firstPick=point;$('status').textContent='Now click the opposite point on the rim.';return;}
  marker=mode==='midpoint'?mul(add(firstPick,point),.5):point;current.marker_basis='explicit geometry pick';mode=null;firstPick=null;refresh();});
 $('center').onclick=()=>{if(!marker||!current)return;const loc=proposed(),cam=camera(loc,current.offset),q=cameraPoint(marker,cam);if(q[2]<=0)return;
  // Shift the eye in its own XY plane. Retain the forward coordinate and rotation.
  const delta=add(mul(cam.basis[0],q[0]),mul(cam.basis[1],q[1]));
  xyz.forEach((axis,i)=>{$('p'+axis).value=(loc.position[i]+delta[i]-current.locator.position[i])*1000;});refresh();};
 $('centerAngle').onclick=()=>{if(!marker||!current)return;const loc=proposed(),direction=sub(marker,loc.position),length=Math.hypot(...direction);if(length<1e-8)return;
  const k=mul(direction,-1/length),z=loc.rotation[2],sz=Math.sin(z),cz=Math.cos(z);
  const r=[Math.asin(Math.max(-1,Math.min(1,k[0]*sz+k[1]*cz))),Math.atan2(-k[0]*cz+k[1]*sz,k[2]),z];
  xyz.forEach((axis,i)=>{$('r'+axis).value=(r[i]-current.locator.rotation[i])*180/Math.PI;});refresh();};
 $('reset').onclick=()=>{for(const id of controls)$(id).value=0;refresh();};
 $('download').onclick=()=>{remember();const plan={format:'skhid-sight-alignment-v1',changes:[...changes.values()]};const a=document.createElement('a');const url=URL.createObjectURL(new Blob([JSON.stringify(plan,null,2)+'\n'],{type:'application/json'}));a.href=url;a.download='sight-alignment-plan.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
 let referenceUrl=null;$('referenceFile').onchange=event=>{if(referenceUrl)URL.revokeObjectURL(referenceUrl);const file=event.target.files[0];if(file){referenceUrl=URL.createObjectURL(file);$('reference').src=referenceUrl;}};
 addEventListener('resize',refresh);const initial=catalog.find(r=>r.id==='a9_91--fit_a9_91_cobra_ekp_8')||catalog.find(r=>r.status!=='unsupported');if(initial){$('pairs').value=initial.id;await select(initial.id);}
}
start().catch(fail);
