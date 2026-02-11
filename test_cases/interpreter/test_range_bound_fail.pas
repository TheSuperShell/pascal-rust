program n;
var r: 0..100;
    tp: string;
procedure func(r: 0..100) begin end;
begin
    readln(tp);
    if tp = 'func' then
        func(101)
    else if tp = 'var' then
        r := 101
end.