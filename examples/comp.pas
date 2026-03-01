program compiled;
    const lower_lim = 1;
        upper_lim = 30;
    var b: integer;

    function fib(num: integer): integer;
    begin
        if num <= 2 then
            exit(num);
        exit(fib(num - 1)  + fib(num - 2))
    end;
begin
    for b := lower_lim to upper_lim do
        writeln(fib(b));
end.