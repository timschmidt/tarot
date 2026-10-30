const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname,'..');
const build = path.join(root,'tmp');
fs.mkdirSync(build,{recursive:true});
const cfg = path.join(build,'fonts.conf');
const escapeXml = s => s.replaceAll('&','&amp;').replaceAll('<','&lt;');
fs.writeFileSync(cfg,`<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "fonts.dtd"><fontconfig><dir>${escapeXml(path.join(root,'fonts'))}</dir><dir>/usr/share/fonts/truetype/dejavu</dir><cachedir>${escapeXml(path.join(build,'font-cache'))}</cachedir></fontconfig>`);
process.env.FONTCONFIG_FILE=cfg;
const sharp = require('sharp');
(async()=>{
 const dir=path.join(root,'screens','svg');
 for(const file of fs.readdirSync(dir).filter(n=>n.endsWith('.svg'))){
   await sharp(path.join(dir,file),{density:144}).resize(720,1280).png().toFile(path.join(root,'screens','png',file.replace('.svg','.png')));
 }
 for(const file of fs.readdirSync(path.join(root,'wallpapers')).filter(n=>n.endsWith('.svg'))){
   await sharp(path.join(root,'wallpapers',file),{density:144}).resize(720,1280).png().toFile(path.join(root,'wallpapers',file.replace('.svg','.png')));
 }
 for(const file of fs.readdirSync(path.join(root,'components')).filter(n=>n.endsWith('.svg'))){
   await sharp(path.join(root,'components',file),{density:144}).resize(1440).png().toFile(path.join(root,'components',file.replace('.svg','.png')));
 }
 console.log('Rendered all screen and wallpaper assets.');
})();
