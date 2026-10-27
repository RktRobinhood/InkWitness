from pathlib import Path
import io, os, html
from PIL import Image, ImageDraw, ImageFont
import cairosvg

ROOT=Path(__file__).resolve().parents[1]
B=ROOT/'assets'/'brand'; P=B/'png'; P.mkdir(parents=True,exist_ok=True)
NAVY='#13213A'; BLUE='#3685F5'; SKY='#8DC6FF'; SLATE='#60728B'; GOLD='#EDBA69'; PAPER='#F8FBFF'; PALE='#EAF3FF'

# The mark is entirely vector-authored and original; no original WordCraft/ArtCraft or Microsoft branding is copied.
# Three readable pictograms: document, ink/quill, and the eye (eyewitness / InkWitness).
inner='''
  <!-- The documented page is the underlying record. -->
  <path d="M39 18.5H81.5L100.5 37.5V101.5C100.5 104.3 98.3 106.5 95.5 106.5H39C36.2 106.5 34 104.3 34 101.5V23.5C34 20.7 36.2 18.5 39 18.5Z" fill="#F9FCFF"/>
  <path d="M81.5 18.5V35.5C81.5 37.2 82.8 38.5 84.5 38.5H100.5" fill="none" stroke="#B5D9FF" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M46 45.5H72M46 55H78" fill="none" stroke="#A7CEFA" stroke-width="4.1" stroke-linecap="round"/>
  <!-- This blue ink feather gives the eye its written provenance. -->
  <path d="M29 27.5C14.5 39.5 9.5 62.5 19 83.5C29.5 75 41.5 58.5 36.3 42.4C34.5 36.5 32.5 31 29 27.5Z" fill="#398AF7"/>
  <path d="M29 27.5C33 45.5 26.1 64.5 17.5 90.5" fill="none" stroke="#D8EAFF" stroke-width="3.2" stroke-linecap="round"/>
  <path d="M20.7 46.8L34.3 53.9M17.5 62.6L29.6 69.7" fill="none" stroke="#1661C8" stroke-width="2.1" stroke-linecap="round" opacity="0.75"/>
  <!-- An eye, rather than a policing shield, represents witnessed development. -->
  <path d="M43 81.1C54.6 66.1 83 66.1 95.2 81.1C83 96.4 55.5 96.4 43 81.1Z" fill="#FFFFFF" stroke="#13213A" stroke-width="4.2" stroke-linejoin="round"/>
  <circle cx="69.2" cy="81.1" r="10.9" fill="#3685F5"/>
  <circle cx="69.2" cy="81.1" r="5.3" fill="#142746"/>
  <circle cx="65.5" cy="77.2" r="2.4" fill="#FFFFFF"/>
  <path d="M20 99.3C27.5 104.5 36.1 106.1 48.5 103.9" fill="none" stroke="#8DC6FF" stroke-width="3.2" stroke-linecap="round"/>
'''

def svg_wrap(content, bg=None, desc='InkWitness logo: an eye, ink feather and document', view='0 0 128 128'):
  back = f'<rect width="128" height="128" rx="27" fill="{bg}"/>' if bg else ''
  return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view}" role="img" aria-label="{html.escape(desc)}"><title>InkWitness</title><desc>{html.escape(desc)}</desc>{back}{content}</svg>\n'''

def write(name, content):
 (B/name).write_text(content,encoding='utf-8')

write('inkwitness-mark.svg',svg_wrap(inner))
write('inkwitness-app-dark.svg',svg_wrap(inner,NAVY))
write('inkwitness-app-light.svg',svg_wrap(inner,'#EAF3FF'))
# For roles, small icons are differentiated by a clear corner chip, not a different logo.
write('inkwitness-write.svg',svg_wrap(inner+'''<circle cx="104" cy="107" r="18" fill="#3685F5" stroke="#FFFFFF" stroke-width="4"/><path d="M97 113L110 100M98 107L104 113M99 111L96 116L101 114" fill="none" stroke="white" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>''',NAVY,'InkWitness Write icon'))
write('inkwitness-review.svg',svg_wrap(inner+'''<circle cx="104" cy="107" r="18" fill="#EDBA69" stroke="#FFFFFF" stroke-width="4"/><path d="M101 98V116L112 107Z" fill="#13213A"/>''',NAVY,'InkWitness Review icon'))
# Dedicated tiny favicon: simple document-eye mark with no microtype or delicate feather detail.
favicon='''<rect width="64" height="64" rx="14" fill="#13213A"/><path d="M18 9H40L50 19V54H18Z" fill="#FFFFFF"/><path d="M40 9V19H50" fill="none" stroke="#9ACBFF" stroke-width="2.8"/><path d="M13 39Q32 18 52 39Q32 59 13 39Z" fill="white" stroke="#13213A" stroke-width="4"/><circle cx="32.5" cy="39" r="10.3" fill="#3685F5"/><circle cx="32.5" cy="39" r="5" fill="#13213A"/><circle cx="29.2" cy="35.7" r="2.1" fill="white"/>'''
write('favicon.svg',f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" role="img" aria-label="InkWitness favicon">{favicon}</svg>\n')

# Wordmark with system-font fallback, all paths for the symbol and live text for text-based accessibility.
# A single blue accent clearly separates Ink/Witness without being a second brand.
lockup_common=f'''<g transform="translate(16,10) scale(.62)">{inner}</g>'''
write('inkwitness-lockup.svg',f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 790 114" role="img" aria-label="InkWitness, every document has a story"><title>InkWitness</title>{lockup_common}<text x="113" y="71" font-family="Inter, Segoe UI, Arial, sans-serif" font-size="54" font-weight="800" letter-spacing="-2.4" fill="{NAVY}">Ink<tspan fill="{BLUE}">Witness</tspan></text><text x="116" y="97" font-family="Inter, Segoe UI, Arial, sans-serif" font-size="13" font-weight="700" letter-spacing="3.1" fill="{SLATE}">EVERY DOCUMENT HAS A STORY.</text></svg>\n''')
write('inkwitness-lockup-on-dark.svg',f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 790 114" role="img" aria-label="InkWitness, every document has a story"><title>InkWitness</title>{lockup_common}<text x="113" y="71" font-family="Inter, Segoe UI, Arial, sans-serif" font-size="54" font-weight="800" letter-spacing="-2.4" fill="white">Ink<tspan fill="{SKY}">Witness</tspan></text><text x="116" y="97" font-family="Inter, Segoe UI, Arial, sans-serif" font-size="13" font-weight="700" letter-spacing="3.1" fill="#BCD3EE">EVERY DOCUMENT HAS A STORY.</text></svg>\n''')
# Icon-only monochrome logo for print / Watermark use, no color dependency.
write('inkwitness-mark-mono.svg',svg_wrap(inner.replace('#398AF7','#13213A').replace('#3685F5','#13213A').replace('#1661C8','#13213A').replace('#8DC6FF','#13213A')))

# Render PNG assets from committed SVG sources, source SVG remains authoritative.
def render_svg(filename,size):
 return Image.open(io.BytesIO(cairosvg.svg2png(bytestring=(B/filename).read_bytes(),output_width=size,output_height=size))).convert('RGBA')
for sz in [16,32,48,64,128,180,192,256,512]:
 src='favicon.svg' if sz <= 32 else 'inkwitness-app-dark.svg'
 img=render_svg(src,sz)
 img.save(P/f'inkwitness-{sz}.png',optimize=True)
render_svg('inkwitness-app-light.svg',512).save(P/'inkwitness-app-light-512.png',optimize=True)
render_svg('inkwitness-write.svg',256).save(P/'inkwitness-write-256.png',optimize=True)
render_svg('inkwitness-review.svg',256).save(P/'inkwitness-review-256.png',optimize=True)
# special Apple touch icon and ico
render_svg('inkwitness-app-dark.svg',180).convert('RGB').save(B/'apple-touch-icon.png',optimize=True)
render_svg('favicon.svg',256).save(B/'favicon.ico',sizes=[(16,16),(32,32),(48,48),(64,64)],format='ICO')

# High quality readme header and social banner. No bundled font files.
font_candidates=['/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf','/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf']
font_bold_candidates=['/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf','/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf']
def getfont(n,bold=False):
 for p in (font_bold_candidates if bold else font_candidates):
  if os.path.isfile(p):return ImageFont.truetype(p,n)
 return ImageFont.load_default()

def palette_rgb(h): return tuple(int(h[i:i+2],16) for i in (1,3,5))
# social preview
im=Image.new('RGB',(1280,640),palette_rgb(NAVY))
d=ImageDraw.Draw(im)
d.rounded_rectangle((70,86,1210,556),radius=34,fill='#1B3458',outline='#385373',width=2)
d.ellipse((895,-70,1420,455),fill='#203B63')
d.ellipse((1070,10,1420,325),fill='#294B77')
mark=render_svg('inkwitness-app-dark.svg',330)
im.paste(mark,(820,156),mark)
d.text((122,170),'Ink',font=getfont(89,True),fill=palette_rgb('#FFFFFF'))
ink_w=d.textbbox((0,0),'Ink',font=getfont(89,True))[2]
d.text((122+ink_w-8,170),'Witness',font=getfont(89,True),fill=palette_rgb('#79B5FF'))
d.text((125,305),'Every document has a story.',font=getfont(32),fill=palette_rgb('#D3E4FA'))
d.line((125,371,688,371),fill='#5B7293',width=2)
d.text((125,407),'WRITE  /  REPLAY  /  UNDERSTAND',font=getfont(21,True),fill='#8ECAFF')
d.text((125,472),'Open-source | Word-compatible | Local-first',font=getfont(19),fill='#BCD3EE')
im.save(B/'social-preview.png',optimize=True)

# Gallery has actual renderings, no invented product screens or services.
prev=Image.new('RGB',(1460,960),'#F5F8FD'); dr=ImageDraw.Draw(prev)
dr.rounded_rectangle((45,48,1415,915),radius=38,fill='white',outline='#DBE7F6',width=3)
dr.text((85,85),'InkWitness',font=getfont(67,True),fill=palette_rgb(NAVY))
dr.text((88,171),'Identity system  /  student editor & teacher review',font=getfont(24),fill=palette_rgb(SLATE))
dr.rounded_rectangle((85,250,520,645),radius=30,fill=palette_rgb(NAVY))
q=render_svg('inkwitness-app-dark.svg',300);prev.paste(q,(150,280),q)
dr.rounded_rectangle((560,250,972,645),radius=30,fill='#E8F2FF')
q=render_svg('inkwitness-app-light.svg',300);prev.paste(q,(615,280),q)
dr.rounded_rectangle((1012,250,1370,645),radius=30,fill='#F6F8FB',outline='#DDE6F0',width=2)
q=render_svg('inkwitness-write.svg',155);prev.paste(q,(1035,310),q)
q=render_svg('inkwitness-review.svg',155);prev.paste(q,(1185,310),q)
dr.text((1090,510),'Write / Review',font=getfont(25,True),fill=palette_rgb(NAVY))
for i,(lab,color) in enumerate([('INK NAVY',NAVY),('WITNESS BLUE',BLUE),('PAPER BLUE','#EAF3FF'),('REVIEW GOLD',GOLD)]):
 x=85+i*332;dr.rounded_rectangle((x,700,x+285,810),radius=18,fill=color,outline='#CEDBEC',width=2)
 dr.text((x+5,825),lab,font=getfont(18,True),fill=palette_rgb(NAVY))
dr.text((88,878),'Original SVG artwork - maintain WordCraft attribution separately.',font=getfont(18),fill=palette_rgb(SLATE))
prev.save(B/'brand-preview.png',optimize=True)

# An approachable local HTML asset gallery, no external requests.
(B/'preview.html').write_text('''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>InkWitness / Brand assets preview</title><style>:root{--navy:#13213A;--blue:#3685F5;--slate:#60728B}*{box-sizing:border-box}body{background:#f4f8fd;color:var(--navy);font:15px/1.5 system-ui,Segoe UI,sans-serif;margin:0}.wrap{max-width:1080px;margin:auto;padding:38px 20px 70px}h1{font-size:34px;margin:0}p{color:#58677e}h2{margin:30px 0 13px;font-size:18px}section{background:#fff;border:1px solid #dce7f5;padding:24px;border-radius:20px;margin-top:18px}.samples{display:grid;grid-template-columns:repeat(auto-fit,minmax(175px,1fr));gap:14px}.cell{min-height:184px;display:flex;flex-direction:column;gap:8px;align-items:center;justify-content:center;background:#eaf3ff;border-radius:12px;padding:17px}.cell.dark{background:#13213a;color:white}.cell img{width:98px;height:98px;object-fit:contain}.cell small{font-size:11px}.lockup{width:min(100%,690px)}.palette{display:flex;gap:10px;flex-wrap:wrap}.sw{padding:12px 18px;border-radius:10px;background:#13213a;color:white}.info{font-size:12px}code{background:#eff4fb;padding:3px 5px;border-radius:5px}</style></head><body><div class="wrap"><h1>InkWitness</h1><p>Identity and icon preview. This page is not the student editor or teacher reviewer.</p><section><img class="lockup" src="inkwitness-lockup.svg" alt="InkWitness wordmark"><p>Every document has a story.</p></section><section><h2>App identity</h2><div class="samples"><div class="cell dark"><img src="inkwitness-app-dark.svg" alt="Dark InkWitness icon"><small>Application / dark</small></div><div class="cell"><img src="inkwitness-app-light.svg" alt="Light InkWitness icon"><small>Application / light</small></div><div class="cell"><img src="inkwitness-write.svg" alt="InkWitness Write icon"><small>Student / Write</small></div><div class="cell"><img src="inkwitness-review.svg" alt="InkWitness Review icon"><small>Teacher / Review</small></div></div></section><section><h2>Favicon sizes</h2><div class="samples"><div class="cell"><img style="width:16px;height:16px" src="png/inkwitness-16.png" alt="16 icon"><small>16 × 16</small></div><div class="cell"><img style="width:32px;height:32px" src="png/inkwitness-32.png" alt="32 icon"><small>32 × 32</small></div><div class="cell"><img style="width:64px;height:64px" src="png/inkwitness-64.png" alt="64 icon"><small>64 × 64</small></div></div></section><section><h2>Palette</h2><div class="palette"><span class="sw" style="background:#13213a">#13213A · Ink Navy</span><span class="sw" style="background:#3685F5">#3685F5 · Witness Blue</span><span class="sw" style="background:#EAF3FF;color:#13213a">#EAF3FF · Paper Blue</span><span class="sw" style="background:#EDBA69;color:#13213a">#EDBA69 · Review Gold</span></div><p class="info">Use review gold only for functional attention markers; do not use it for default warning or misconduct verdicts.</p></section><section><h2>Implementation note</h2><p>Use SVG sources when possible and raster files only when the target needs them. Brand logo replacements do not imply replacing the editing ribbon's ordinary functional icons. Preserve upstream attribution and separate WordCraft's legally restricted marks from this fork.</p><p class="info">See <code>docs/BRANDING.md</code> at repository root for implementation requirements.</p></section></div></body></html>''',encoding='utf-8')

print('Built',len(list(B.rglob('*'))),'brand entries at',B)
