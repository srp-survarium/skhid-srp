"""Strict temporary-component operand decoding for pinned SM4 audit programs."""
def operand(op,at):
    tok=op[at];at+=1
    ext=tok
    while ext>>31:
        ext=op[at];at+=1
    typ=(tok>>12)&255;dim=(tok>>20)&3;indices=[]
    for i in range(dim):
        assert (tok>>(22+3*i))&7==0, 'relative operand not audited'
        indices.append(op[at]);at+=1
    count=tok&3
    if typ==4:
        at+=1 if count==1 else 4
    comps=set()
    if typ==0:
        assert len(indices)==1
        if count==1:comps={0}
        elif count==2:
            mode=(tok>>2)&3
            if mode==0:comps={c for c in range(4) if tok&(1<<(4+c))}
            elif mode==1:comps={(tok>>(4+2*c))&3 for c in range(4)}
            elif mode==2:comps={(tok>>4)&3}
            else:raise AssertionError('operand mode')
        else:raise AssertionError('temp dimensions')
    return at,{(indices[0],c) for c in comps}

def operands(op):
    at=1;result=[]
    while at<len(op):
        at,used=operand(op,at);result.append(used)
    assert at==len(op)
    return result

