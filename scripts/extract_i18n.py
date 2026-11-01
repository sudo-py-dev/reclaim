"""Extract English message IDs for catalog review; handles Rust raw strings."""
import json, re
from pathlib import Path

def literals(text):
    i=0
    while i<len(text):
        if text.startswith('//',i):
            end=text.find('\n',i); i=len(text) if end<0 else end+1; continue
        if text.startswith('/*',i):
            end=text.find('*/',i+2); i=len(text) if end<0 else end+2; continue
        raw=re.match(r'r(#+)?"',text[i:])
        if raw:
            marker='"'+(raw[1] or ''); start=i+len(raw[0]); end=text.find(marker,start)
            if end<0: break
            yield text[start:end]; i=end+len(marker); continue
        char=re.match(r"'(?:\\.|[^'\\])'",text[i:])
        if char: i+=len(char[0]); continue
        if text[i]=='"':
            start=i; i+=1
            while i<len(text):
                if text[i]=='\\': i+=2; continue
                if text[i]=='"': break
                i+=1
            value=text[start:i+1]
            try: yield json.loads(value)
            except ValueError: pass
        i+=1

def messages():
    values=set()
    for p in Path('src').glob('*.rs'):
        if p.name in ('ui_tests.rs','i18n.rs'): continue
        text=p.read_text().split('#[cfg(test)]')[0].split('#[cfg(all(test,')[0]
        for s in literals(text):
            if len(s)>2 and re.search('[A-Za-z]',s) and (' ' in s or s in ['Back','Refresh','System','Light','Dark','Minimize','Maximize','Close','Reclaim','Scan','Language','English','Hebrew','Pictures','Documents','Archives','Audio','Video','Applications','Icons','Appearance','Recover']):
                if any(ord(c)<32 for c in s) or s.startswith(('http','/dev/','<','\\\\')): continue
                if not re.search(r'[A-Za-z]{2,}[^{}]* ',s) and s.startswith('{'): continue
                values.add(s)
    return sorted(values)

if __name__=='__main__':
    print(json.dumps(messages(),ensure_ascii=False,indent=2))
