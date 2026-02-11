program n;
const zer = 0.0;
var i: real;
    tp: string;
begin
    readln(tp);
    if tp = 'int/int' then
        i := 100 / 0
    else if tp = 'real/int' then
        i := 100.0 / 0
    else if tp = 'int div int' then
        i := 100 div 0
    else if tp = 'real div int' then
        i := 100.0 div 0
    else if tp = 'int div real' then
        i := 100 div zer
    else if tp = 'real div real' then
        i := zer div zer
end.