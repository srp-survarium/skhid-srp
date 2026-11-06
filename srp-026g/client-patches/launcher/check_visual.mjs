// Compare actual rendered pages with the player's original 0.26g HTML/CSS.
// Node 22+, Firefox running with --headless --remote-debugging-port 9448.
// Usage: node check_visual.mjs <game/launcher> <fixture-directory> [port]
// Export fixtures as documented in README.md first. No npm dependencies.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';

const [original, output, port = '9448'] = process.argv.slice(2);
if (!original || !output) throw Error('Expected original game/launcher and fixture directory');
const socket = new WebSocket(`ws://127.0.0.1:${port}/session`);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let id = 0;
const pending = new Map();
socket.onmessage = event => {
    const message = JSON.parse(event.data);
    const request = pending.get(message.id);
    if (!request) return;
    pending.delete(message.id);
    clearTimeout(request.timer);
    if (message.type === 'error') request.reject(Error(JSON.stringify(message)));
    else request.resolve(message.result);
};
const send = (method, params = {}) => new Promise((resolve, reject) => {
    const next = ++id;
    const timer = setTimeout(() => { pending.delete(next); reject(Error(`Timed out: ${method}`)); }, 20000);
    pending.set(next, {resolve, reject, timer});
    socket.send(JSON.stringify({id: next, method, params}));
});
try {
    await send('session.new', {capabilities: {alwaysMatch: {browserName: 'firefox'}}});
    const {context} = await send('browsingContext.create', {type: 'tab'});
    await send('browsingContext.setViewport', {context, viewport: {width: 899, height: 335}, devicePixelRatio: 1});
    async function evaluate(expression) {
        const result = await send('script.evaluate', {expression, target: {context}, awaitPromise: true});
        if (result.type === 'exception') throw Error(JSON.stringify(result));
        return result.result.value;
    }
    async function settle() {
        await evaluate(`document.fonts.ready.then(() => new Promise(resolve =>
            requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 100)))))`);
    }
    async function navigate(file) {
        await send('browsingContext.navigate', {context, url: pathToFileURL(path.resolve(file)).href, wait: 'complete'});
        await settle();
    }
    async function screenshot(name) {
        await settle();
        const {data} = await send('browsingContext.captureScreenshot', {context});
        await fs.writeFile(path.join(output, `${name}.png`), Buffer.from(data, 'base64'));
        return data;
    }
    const geometry = (left, tiles, captions) => `JSON.stringify({
        left: rect(document.querySelector(${JSON.stringify(left)})),
        tiles: [...document.querySelectorAll(${JSON.stringify(tiles)})].map(rect),
        captions: [...document.querySelectorAll(${JSON.stringify(captions)})].map(rect)
    }); function rect(e) {const r=e.getBoundingClientRect();return [r.x,r.y,r.width,r.height]}`;
    await navigate(path.join(original, 'index-en.html'));
    const reference = JSON.parse(await evaluate(geometry('#left-container', '#right-container a', '#right-container span')));
    await screenshot('original');
    // Keep the original CSS and artwork. Only remove news content and replace
    // caption text, so all the pixels left have an equivalent in Skhid.
    await evaluate(`document.querySelector('#left-container').innerHTML='';
        document.querySelectorAll('#right-container img').forEach(e=>e.style.visibility='hidden');
        document.querySelectorAll('#right-container span').forEach(e=>e.textContent='SURVARIUM · СХІД');`);
    const referencePixels = await screenshot('reference-skin');
    const files = (await fs.readdir(output)).filter(f => /^(online-|index-).*\.html$/.test(f)).sort();
    assert.equal(files.length, 24, 'Expected 14 server fixtures and all 10 local pages');
    const results = [];
    for (const file of files) {
        await navigate(path.join(output, file));
        assert.deepEqual(JSON.parse(await evaluate(geometry('#left', '.tile', '#right .cap'))), reference, file);
        const issues = JSON.parse(await evaluate(`JSON.stringify((()=>{
            const issues=[];
            if(document.documentElement.scrollWidth>899||document.documentElement.scrollHeight>335) issues.push('page overflow');
            const discord=document.querySelector('a.discord');
            if(!discord || discord.href!=='https://discord.gg/QUmJg3gBhg' || discord.target!=='_blank') issues.push('missing external Discord link');
            if(discord && (discord.getBoundingClientRect().width===0 || discord.getBoundingClientRect().right>883)) issues.push('Discord link clipped');
            const left=document.querySelector('#left'), cap=left.querySelector('.cap');
            // The board and the matches are two tabs: the one that is not shown has no size.
            const tabs=[...left.querySelectorAll('#board,#live')].filter(tab=>tab.getBoundingClientRect().height>0);
            if(left.querySelector('#board') && (tabs.length!==1 || tabs[0].id!==(left.className==='live'?'live':'board'))) issues.push('wrong tab shown');
            for(const tab of tabs) for(const child of tab.children) if(child.getBoundingClientRect().right>left.getBoundingClientRect().right || child.getBoundingClientRect().bottom>cap.getBoundingClientRect().top) issues.push('board overflow');
            const heading=left.querySelector('h1');
            if(heading.scrollWidth>heading.clientWidth || heading.getBoundingClientRect().height!==30) issues.push('heading overflow');
            document.querySelectorAll('.tile').forEach(tile=>{
                const cap=tile.querySelector('.cap');
                for(const child of tile.children) if(child!==cap && child.getBoundingClientRect().bottom>cap.getBoundingClientRect().top) issues.push('caption covers content');
                if(cap.scrollWidth>cap.clientWidth) issues.push('caption truncated');
            });
            return issues;
        })())`));
        assert.deepEqual(issues, [], file);
        await screenshot(file.replace('.html', ''));
        // Same normalization as the reference. Warnings intentionally use amber;
        // normalize that content state too, without changing geometry or fonts.
        await evaluate(`document.querySelector('#left').innerHTML='';
            document.querySelectorAll('.tile').forEach(tile=>{
                for(const child of tile.children) if(!child.classList.contains('cap')) child.remove();
                const cap=tile.querySelector('.cap');cap.textContent='SURVARIUM · СХІД';cap.classList.remove('warn');
            });`);
        await settle();
        const {data: actualPixels} = await send('browsingContext.captureScreenshot', {context});
        const difference = await evaluate(`(async()=>{
            async function pixels(data) {
                const img=new Image(); img.src='data:image/png;base64,'+data; await img.decode();
                const canvas=document.createElement('canvas');canvas.width=899;canvas.height=335;
                const ctx=canvas.getContext('2d');ctx.drawImage(img,0,0);return ctx.getImageData(0,0,899,335).data;
            }
            const a=await pixels(${JSON.stringify(referencePixels)}),b=await pixels(${JSON.stringify(actualPixels)});
            let changed=0;for(let i=0;i<a.length;i+=4)if(a[i]!==b[i]||a[i+1]!==b[i+1]||a[i+2]!==b[i+2]||a[i+3]!==b[i+3])changed++;
            return changed;
        })()`);
        assert.equal(difference, 0, `${file}: skin differs from original pixels`);
        results.push({file, geometry: 'identical', changedSkinPixels: difference, overflow: false});
    }
    await fs.writeFile(path.join(output, 'comparison.json'), JSON.stringify({reference, results}, null, 2)+'\n');
    console.log(`PASS: ${files.length} pages; original geometry and skin pixels identical; no overlapping content.`);
    await send('session.end');
} finally {
    for (const request of pending.values()) clearTimeout(request.timer);
    socket.close();
}
