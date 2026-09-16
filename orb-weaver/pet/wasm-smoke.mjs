import fs from 'node:fs';
import assert from 'node:assert/strict';
const binary=new URL('./web/orb_weaver_pet.wasm',import.meta.url);
const {instance:{exports:pet}}=await WebAssembly.instantiate(fs.readFileSync(binary),{});
const snapshot=()=>new Float32Array(pet.memory.buffer,pet.pet_frame(),pet.pet_frame_len());
pet.pet_mode(1);pet.pet_environment(.6,.2,1);
for(let i=0;i<3600;i++){
 pet.pet_target(Math.sin(i*.002)*2,Math.cos(i*.003)*1.5);pet.pet_update(1/60);
 const f=snapshot();assert(f.every(Number.isFinite));assert.equal(f[0],1);assert.equal(f[5],8);
 let moving=0;for(let j=0;j<8;j++)moving+=f[16+22*3+j*14+12];assert(moving<=4);
}
pet.pet_mode(3);for(let i=0;i<1200;i++)pet.pet_update(1/60);
assert.equal(snapshot()[14],1);assert.equal(snapshot()[6],195);
pet.pet_clear_web();pet.pet_update(1/60);assert.equal(snapshot()[6],0);
pet.pet_reset();assert.equal(snapshot()[6],0);assert(snapshot().every(Number.isFinite));
console.log('WASM: walking, terrain, support, silk completion, clear and reset passed.');
