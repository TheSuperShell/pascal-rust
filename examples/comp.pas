program compiled;
    const lower_lim = 1;
        upper_lim = 30;
    var b: integer = 5;
    var o: integer = 0;

    function fib(num: integer; out extra: integer): integer;
    begin
        extra := extra + num;
        if num <= 2 then
            exit(num);
        exit(fib(num - 1, extra)  + fib(num - 2, extra))
    end;
begin
    for b := lower_lim to upper_lim do
        writeln(fib(b, o));
    writeln(o);
end.