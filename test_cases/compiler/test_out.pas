program n;
procedure out_test(a: integer; out b: integer)
begin
    b := b + a;
end;

procedure out_recurs(out b: integer)
begin
    if b <= 25 then
    begin
        b := b + 5;
        out_recurs(b);
    end
end;
    var res: integer = 10;
begin
    out_test(20, res);
    writeln(res);
    res := 0;
    out_recurs(res);
    writeln(res);
end.

