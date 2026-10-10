#!/usr/bin/env python3
"""Regenerates expat_corpus.txt, the corpus of tests/xml_expat_corpus.rs.

Runs a fixed set of hand-written documents plus deterministic mutations of them through expat
(Python's pyexpat, expat 2.6) the way Skia's SkXMLParser drives it, and records the events:

  D <hex document>
  R ok|err
  S <hex name> | A <hex name> <hex value> | E <hex name> | T <hex text>   (the events, in order)
  (blank line)

SkXMLParser has no unknown-encoding handler, but pyexpat installs one, so documents that declare
an encoding expat does not have built in are recorded as errors here.

Usage: gen_expat_corpus.py <out-file> [mutations-per-seed (default 12)] [rng-seed (default 1)]
"""
import sys
import random

SEEDS = [
    b'<a/>',
    b'<a></a>',
    b'<a>text</a>',
    b'<?xml version="1.0"?><a/>',
    b'<?xml version="1.0" encoding="UTF-8"?><a/>',
    b'<?xml version="1.0" encoding="utf-8" standalone="yes"?><a/>',
    b'<?xml version="1.0" encoding="ISO-8859-1"?><a>\xe9\xff</a>',
    b'<?xml version="1.0" encoding="US-ASCII"?><a>abc</a>',
    b'<?xml version="1.0" encoding="UTF-16"?><a/>',
    b'<?xml version="1.0" encoding="Shift_JIS"?><a/>',
    b'<?xml version="1.1"?><a/>',
    b'<?xml version="2.0"?><a/>',
    b'<?xml encoding="UTF-8"?><a/>',
    b'<?xml version="1.0" standalone="maybe"?><a/>',
    b'<?xml version="1.0" encoding="UTF-8" standalone="no"?>\n<!-- c -->\n<a x="1" y=\'2\'>t<b/>u</a>\n<!-- end -->\n',
    b' <?xml version="1.0"?><a/>',
    b'<!-- c --><?xml version="1.0"?><a/>',
    b'<a><?xml version="1.0"?></a>',
    b'<a/><?XML foo?>',
    b'<?XML foo?><a/>',
    b'<?xml-stylesheet href="x"?><a/>',
    b'<?pi?><a/>',
    b'<?pi data ?><a/>',
    b'<? pi ?><a/>',
    b'<a>&lt;&gt;&amp;&quot;&apos;</a>',
    b'<a>&#65;&#x41;&#X41;&#x;&#;</a>',
    b'<a>&#0;</a>',
    b'<a>&#x110000;</a>',
    b'<a>&#xD800;</a>',
    b'<a>&#xFFFE;</a>',
    b'<a>&#x1F600;</a>',
    b'<a>&#10;&#13;&#9;</a>',
    b'<a>&#8;</a>',
    b'<a>&foo;</a>',
    b'<a>&amp</a>',
    b'<a>& amp;</a>',
    b'<a>&#65</a>',
    b'<a x="&lt;&#65;&#x42;&amp;&quot;&apos;"/>',
    b'<a x="&#10;&#13;&#9;"/>',
    b'<a x="1\n2\r3\r\n4\t5"/>',
    b'<a x="&foo;"/>',
    b'<a x="<"/>',
    b'<a x=">"/>',
    b'<a x="a&b"/>',
    b"<a x='\"'/>",
    b'<a x="\'"/>',
    b'<a x=1/>',
    b'<a x/>',
    b'<a x=/>',
    b'<a x="1" x="2"/>',
    b'<a x="1"y="2"/>',
    b'<a x = "1" y\t=\n"2"/>',
    b'<a  x="1"  />',
    b'<a / >',
    b'<a/ >',
    b'<a x="1"/ >',
    b'<a>\r\n\r\n\n\r</a>',
    b'<a>  \n  </a>',
    b'<a><![CDATA[<>&]]></a>',
    b'<a><![CDATA[]]></a>',
    b'<a><![CDATA[a\r\nb\rc]]></a>',
    b'<a><![CDATA[x</a>',
    b'<a><![CDATA[ ]] > ]]></a>',
    b'<a>x]]>y</a>',
    b'<a>x]]y</a>',
    b'<a>x]>y</a>',
    b'<![CDATA[x]]><a/>',
    b'<a/><![CDATA[x]]>',
    b'<a><!-- c --></a>',
    b'<a><!-- a -- b --></a>',
    b'<a><!--- c ---></a>',
    b'<a><!----></a>',
    b'<a><!-- c </a>',
    b'<a><!-></a>',
    b'<a>--></a>',
    b'<a/>text',
    b'text<a/>',
    b'<a/>   \n  ',
    b'<a/><b/>',
    b'<a/><!-- c --><?pi?>',
    b'<a></b>',
    b'<a><b></a></b>',
    b'<a><b></b>',
    b'<a>',
    b'</a>',
    b'<a',
    b'<',
    b'',
    b'   ',
    b'\n\n<a/>',
    b'<a>\x01</a>',
    b'<a>\x0b</a>',
    b'<a>\x7f</a>',
    b'<a>\xc2\x80</a>',
    b'<a>\xef\xbf\xbe</a>',
    b'<a>\xef\xbf\xbf</a>',
    b'<a>\xed\xa0\x80</a>',
    b'<a>\xf4\x90\x80\x80</a>',
    b'<a>\xc0\x80</a>',
    b'<a>\xe2\x82</a>',
    b'<a>\xff</a>',
    b'<a>\xf0\x9f\x98\x80</a>',
    b'\xef\xbb\xbf<a/>',
    b'\xef\xbb\xbf<?xml version="1.0"?><a/>',
    b'\xef\xbb\xbf\xef\xbb\xbf<a/>',
    b'<a>\xef\xbb\xbf</a>',
    b'\xff\xfe<\x00a\x00/\x00>\x00',
    b'\xfe\xff\x00<\x00a\x00/\x00>',
    b'<\x00a\x00/\x00>\x00',
    b'\x00<\x00a\x00/\x00>',
    b'\xff\xfe<\x00?\x00x\x00m\x00l\x00 \x00v\x00e\x00r\x00s\x00i\x00o\x00n\x00=\x00"\x001\x00.\x000\x00"\x00?\x00>\x00<\x00a\x00>\x00\xe9\x00<\x00/\x00a\x00>\x00',
    b'\xff\xfe<\x00a\x00>\x00\x3d\xd8\x00\xde<\x00/\x00a\x00>\x00',
    b'\xff\xfe<\x00a\x00>\x00\x3d\xd8<\x00/\x00a\x00>\x00',
    b'\xff\xfe<\x00a\x00>\x00',
    b'\xff\xfe<\x00a\x00/\x00>',
    b'<a\xc3\xa9/>',
    b'<\xc3\xa9/>',
    b'<\xc3\x97/>',
    b'<a\xc3\x97/>',
    b'<a\xc2\xb7/>',
    b'<\xc2\xb7/>',
    b'<\xe4\xb8\xad\xe6\x96\x87/>',
    b'<a \xe4\xb8\xad="1"/>',
    b'<a:b/>',
    b'<a:b:c/>',
    b'<:a/>',
    b'<a:/>',
    b'<a xmlns:x="u" x:y="1"/>',
    b'<a xmlns="u"/>',
    b'<x:a/>',
    b'<1a/>',
    b'<-a/>',
    b'<.a/>',
    b'<a-b.c_d1/>',
    b'<a b-c="1" _d=\'2\'/>',
    b'<a xml:space="preserve"/>',
    b'<a xml:lang="en"/>',
    b'<!DOCTYPE a><a/>',
    b'<!DOCTYPE a ><a/>',
    b'<!DOCTYPE a []><a/>',
    b'<!DOCTYPE a [ ]><a/>',
    b'<!DOCTYPE a SYSTEM "a.dtd"><a/>',
    b"<!DOCTYPE a SYSTEM 'a.dtd'><a/>",
    b'<!DOCTYPE a PUBLIC "-//X//Y" "a.dtd"><a/>',
    b'<!DOCTYPE a PUBLIC "-//X//Y"><a/>',
    b'<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg/>',
    b'<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "x.dtd"><svg>&nbsp;</svg>',
    b'<!DOCTYPE a SYSTEM "a.dtd"><a>&foo;</a>',
    b'<!DOCTYPE a SYSTEM "a.dtd"><a x="&foo;"/>',
    b'<!DOCTYPE a [<!ENTITY e "x">]><a>&e;</a>',
    b'<!DOCTYPE a [<!ENTITY e "x">]><a/>',
    b'<!DOCTYPE a [<!ENTITY e SYSTEM "x.ent">]><a/>',
    b'<!DOCTYPE a [<!ENTITY % e "x">]><a/>',
    b'<!DOCTYPE a [<!ENTITY lt "&#38;#60;">]><a/>',
    b'<!DOCTYPE a [<!ENTITY a "x"><!ENTITY a "y">]><a/>',
    b'<!DOCTYPE a [<!ENTITY e "x">]><a>&e;</a>',
    b'<!DOCTYPE a [<!ELEMENT a EMPTY>]><a/>',
    b'<!DOCTYPE a [<!ELEMENT a (#PCDATA)>]><a>t</a>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d">]><a x="1"/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA #IMPLIED>]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x NMTOKEN " v ">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x NMTOKENS "a  b">]><a x="  p   q  "/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA #FIXED "f">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x ID #IMPLIED>]><a x=" i "/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d" y CDATA "e">]><a z="1"/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d">]><a x="1" x="2"/>',
    b'<!DOCTYPE a [<!-- c --><?pi?>]><a/>',
    b'<!DOCTYPE a [<!-- c -- d -->]><a/>',
    b'<!DOCTYPE a [%x;]><a/>',
    b'<!DOCTYPE a [<!ENTITY % x "<!ENTITY e \'v\'>"> %x;]><a/>',
    b'<!DOCTYPE a [<!NOTATION n SYSTEM "x">]><a/>',
    b'<!DOCTYPE a [<!FOO>]><a/>',
    b'<!DOCTYPE a [<!ELEMENT a ANY>]>\n<a>&#65;<b/></a>',
    b'<!DOCTYPE b><a/>',
    b'<!DOCTYPE a><!DOCTYPE a><a/>',
    b'<a><!DOCTYPE a></a>',
    b'<!doctype a><a/>',
    b'<!DOCTYPE><a/>',
    b'<!DOCTYPE a [',
    b'<!DOCTYPE a [<!ENTITY e "x>]><a/>',
    b'<!DOCTYPE a [<!ENTITY e "x">]]><a/>',
    b'<!-- c --><!DOCTYPE a><a/>',
    b'<a/><!DOCTYPE a>',
    b'<a>\xe2\x82\xac</a>',
    b'<a x="\xe2\x82\xac"/>',
    b'<a x="\x01"/>',
    b'<a x="\xef\xbf\xbe"/>',
    b'<a x="&#1;"/>',
    b'<a><b x="1"/><b x="2"/></a>',
    b'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10"><g id="g"><rect x="0" y="0" width="1" height="1" fill="#f00"/></g><use xlink:href="#g"/></svg>',
    b'<svg><style><![CDATA[ .a { fill: red } ]]></style><text>hello &amp; <tspan>bye</tspan></text></svg>',
    b'<svg><text>  a  b  </text></svg>',
    b'<a>\t</a>',
    b'<a>a\rb</a>',
    b'<a x="a\rb"/>',
    b'<a\n x="1"\n/>',
    b'<a\x0cx="1"/>',
    b'<a x="1" \x00/>',
    b'<a x="\x00"/>',
    b'<a>\x00</a>',
    b'<a/>\x00',
    b'<a x="1"\xc2\xa0/>',
    b'<a>\xc2\xa0</a>',
    b'<a>\xe2\x80\xa8</a>',
    b'<a>\xc2\x85</a>',
]

DTD_SEEDS = [
    b'<!DOCTYPE a [%x;]><a>&foo;</a>', b'<!DOCTYPE a [%x; <!ATTLIST a x CDATA "d">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d"> %x;]><a/>', b'<!DOCTYPE a [%x; <!ENTITY e "v">]><a/>',
    b'<!DOCTYPE a [<!ELEMENT %x; EMPTY>]><a/>', b'<!DOCTYPE a [<!ENTITY amp "v">]><a>&amp;</a>',
    b'<!DOCTYPE a [<!ENTITY lt SYSTEM "x" NDATA n>]><a/>', b'<!DOCTYPE a[]><a/>',
    b'<!DOCTYPE a SYSTEM "x"[]><a/>', b'<!DOCTYPE a [<!ELEMENT a (b,c)*>]><a/>',
    b'<!DOCTYPE a [<!ELEMENT a (b|c)>]><a/>', b'<!DOCTYPE a [<!ELEMENT a (b)? >]><a/>',
    b'<!DOCTYPE a [<!ELEMENT a ((b|c)+,d?)>]><a/>', b'<!DOCTYPE a [<!ELEMENT a (#PCDATA|b|c)*>]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x (u|v) "u" y NOTATION (n|m) #IMPLIED z ID #REQUIRED>]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA #FIXED "f">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "&amp;&#65;&lt;">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST a x NMTOKENS "\n\tz  y ">]><a x="  p \n q "/>',
    b'<!DOCTYPE a [<!ATTLIST a x NMTOKENS "&#32;z&#32;">]><a x="&#32;&#32;q&#32;"/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d" y CDATA "e">]><a z="1"/>',
    b'<!DOCTYPE a [<!ATTLIST a x CDATA "d"><!ATTLIST a x CDATA "e">]><a/>',
    b'<!DOCTYPE a [<!ATTLIST b x CDATA "d">]><a><b/></a>',
    b'<?xml version="1.0" standalone="yes"?><!DOCTYPE a [%x; <!ATTLIST a x CDATA "d">]><a>&foo;</a>',
    b'<?xml version="1.0" standalone="yes"?><!DOCTYPE a SYSTEM "x"><a>&foo;</a>',
    b'<?xml version="1.0" standalone="no"?><!DOCTYPE a SYSTEM "x"><a>&foo;</a>',
    b'<!DOCTYPE a [<!NOTATION n SYSTEM "x"><!NOTATION m PUBLIC "p" "q"><!NOTATION o PUBLIC "p">]><a/>',
    b'<!DOCTYPE a [<!-- c --><?pi x?><!ELEMENT a ANY>]>\n<a/>',
    b'<!DOCTYPE a PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd" [<!ATTLIST svg xmlns:xlink CDATA #FIXED "http://www.w3.org/1999/xlink">]><svg/>',
    b'<!DOCTYPE a [<!ENTITY % e "x">]><a/>', b'<!DOCTYPE a [<!ENTITY e SYSTEM "x">]><a/>',
    b'<!DOCTYPE a [<!ENTITY e PUBLIC "x" "y" NDATA n>]><a/>',
]

def u16(s, le=True, bom=None):
    b = s.encode('utf-16-le' if le else 'utf-16-be')
    if bom == 'le':
        b = b'\xff\xfe' + b
    if bom == 'be':
        b = b'\xfe\xff' + b
    return b

ENC_SEEDS = []
for enc in [None, 'UTF-16', 'utf-16', 'UTF-16LE', 'UTF-16BE', 'UTF-8', 'ISO-8859-1', 'iso-8859-1', 'US-ASCII', 'latin1']:
    decl = '' if enc is None else '<?xml version="1.0" encoding="%s"?>' % enc
    body = decl + '<a x="é">é&#233;\U0001F600</a>'
    for le, bom in [(True, 'le'), (False, 'be'), (True, None), (False, None)]:
        ENC_SEEDS.append(u16(body, le, bom))
    ENC_SEEDS.append(b'\xef\xbb\xbf' + body.encode('utf-8'))
    ENC_SEEDS.append(body.encode('utf-8'))
    try:
        ENC_SEEDS.append(body.replace('\U0001F600', '').encode('latin1'))
    except UnicodeEncodeError:
        pass
MISC_SEEDS = [
    b'<?xml version="abc"?><a/>', b'<?xml version=""?><a/>', b'<?xml  version = "1.0"  ?><a/>',
    b'<?xml version="1.0"encoding="UTF-8"?><a/>', b'<?xml version="1.0" standalone="yes" encoding="UTF-8"?><a/>',
    b'<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><a/>', b'<?xml version="1.0" encoding="8bit"?><a/>',
    b'<?xml version="1.0" encoding="UTF-8 "?><a/>', b'<?xml?><a/>', b'<?xml ?><a/>', b'<?xmlfoo?><a/>',
    b'<?xml-foo?><a/>', b'<?Xml?><a/>', b'<?xml:a?><a/>', b'<?x ??><a/>', b'<?x?>?><a/>', b'<?1?><a/>',
    b'<a><?xml-stylesheet x?></a>', b'<a><?XML x?></a>',
    b'<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">\r\n\t<rect\twidth="10"\r\n height="10"/>\r\n</svg>\r\n',
    b'<svg><g><g><g><g><g><g><g><g><g><g/></g></g></g></g></g></g></g></g></g></svg>',
    b'<a>text<b>more</b>tail<c/><![CDATA[cd]]>end<!-- c -->after</a>',
    b'<a x="a>b" y=\'a"b\' z="a\'b"/>',
    b'<a x="  a   b  "/>', b'<a x="&#32;a&#32;"/>', b'<a x="a&#10;b&#9;c&#13;d"/>',
    b'<a>&amp;&amp;&lt;&#38;#60;&#x26;</a>', b'<a>x&#x1F600;y&#128512;z</a>',
    b'<a>\xc3\xa9\xe4\xb8\xad\xf0\x9f\x98\x80</a>', b'<\xc3\xa9\xe4\xb8\xad a\xc3\xa9="1"/>',
    b'<a>\n\r\n\r\r\n\n</a>', b'<a x="\n\r\n\r\r\n\n"/>',
    b'<a><![CDATA[a]]><![CDATA[b]]]]><![CDATA[>]]></a>', b'<a><![CDATA[\r\n\r]]></a>',
    b'<a/><!-- c -->\n<?p q?>\n', b'<!-- c --><?p q?><!DOCTYPE a><!-- d --><a/>',
    b'<a:b xmlns:a="u" a:c="1"><a:d/></a:b>', b'<a b:c="1" b:c="2"/>',
    b'<a xml:lang="en" xml:space="preserve" xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="#x"/>',
    b'<a>\xcc\x81</a>', b'<\xcc\x81a/>', b'<a\xcc\x81/>', b'<a\xe2\x80\xbf/>', b'<a\xe2\x80\x8b/>',
    b'<a>\xf4\x8f\xbf\xbf</a>', b'<a>\xf4\x8f\xbf\xbe</a>', b'<a>\xef\xb7\x90</a>', b'<a>\xef\xb7\xaf</a>',
]
SEEDS = SEEDS + DTD_SEEDS + ENC_SEEDS + MISC_SEEDS

INSERTS = [
    b'<', b'>', b'&', b'&amp;', b'&#65;', b'&#x1F600;', b'"', b"'", b'=', b'/', b' ', b'\n', b'\r', b'\t',
    b'<![CDATA[', b']]>', b'<!--', b'-->', b'--', b'<?', b'?>', b'<!DOCTYPE a [', b']>', b'<!ENTITY e "x">',
    b'\xc3\xa9', b'\xef\xbb\xbf', b'\x00', b'\x01', b'\xff', b'\xc3', b'\xe2\x82', b':', b'xml', b'<?xml version="1.0"?>',
    b'x="1"', b' x="1"', b'<b/>', b'</a>', b'<a>', b'\xc3\x97', b'\xc2\xb7',
]


def mutate(rng, doc):
    d = bytearray(doc)
    for _ in range(rng.choice([1, 1, 1, 2, 3])):
        k = rng.randrange(5)
        if k == 0 and d:
            del d[rng.randrange(len(d))]
        elif k == 1:
            d.insert(rng.randrange(len(d) + 1), rng.randrange(256))
        elif k == 2:
            ins = rng.choice(INSERTS)
            i = rng.randrange(len(d) + 1)
            d[i:i] = ins
        elif k == 3 and d:
            i = rng.randrange(len(d))
            j = min(len(d), i + rng.randrange(1, 6))
            del d[i:j]
        elif k == 4 and len(d) > 1:
            i = rng.randrange(len(d))
            j = rng.randrange(len(d))
            d[i], d[j] = d[j], d[i]
    return bytes(d)


import xml.parsers.expat as expat


class Stop(Exception):
    pass


import re

BUILTIN = {'utf-8', 'utf-16', 'iso-8859-1', 'us-ascii', 'utf-16be', 'utf-16le'}
ENC_RE = re.compile(rb'^(?:\xef\xbb\xbf)?<\?xml[ \t\r\n][^>]*?encoding[ \t\r\n]*=[ \t\r\n]*(?:"([^"]*)"|\'([^\']*)\')')


def unknown_encoding(doc: bytes) -> bool:
    # pyexpat installs an unknown-encoding handler that SkXMLParser does not have.
    m = ENC_RE.match(doc)
    if not m:
        return False
    name = (m.group(1) if m.group(1) is not None else m.group(2)).decode('latin1').lower()
    return name not in BUILTIN


def run(doc: bytes):
    if unknown_encoding(doc):
        return False, []
    p = expat.ParserCreate()
    p.ordered_attributes = True
    events = []
    text = bytearray()

    def flush():
        if text:
            events.append(('T', bytes(text)))
            text.clear()

    def start(name, attrs):
        flush()
        events.append(('S', name.encode()))
        for i in range(0, len(attrs), 2):
            events.append(('A', attrs[i].encode(), attrs[i + 1].encode()))

    def end(name):
        flush()
        events.append(('E', name.encode()))

    def chars(data):
        text.extend(data.encode())

    def entity(*a):
        raise Stop()

    p.StartElementHandler = start
    p.EndElementHandler = end
    p.CharacterDataHandler = chars
    p.EntityDeclHandler = entity
    try:
        p.Parse(doc, True)
        ok = True
    except (expat.ExpatError, Stop, ValueError, LookupError):
        ok = False
    return ok, events


def main():
    out = sys.argv[1]
    per = int(sys.argv[2]) if len(sys.argv) > 2 else 12
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    rng = random.Random(seed)
    docs = list(SEEDS)
    for s in SEEDS:
        for _ in range(per):
            docs.append(mutate(rng, s))
    seen = set()
    with open(out, 'w') as f:
        for doc in docs:
            if doc in seen or not doc:
                continue
            seen.add(doc)
            ok, events = run(doc)
            f.write('D ' + doc.hex() + '\n')
            f.write('R ' + ('ok' if ok else 'err') + '\n')
            for ev in events:
                f.write(ev[0] + ' ' + ' '.join(x.hex() for x in ev[1:]) + '\n')
            f.write('\n')
    print(len(seen), 'documents')


main()
