program n;
    var a: int64;
        b: integer;
begin
    a := 3000000000;
    b := 1;
    a := b + a;
    writeln(a);
    a := 3000000000;
    b := a + b;
    writeln(b);
end.