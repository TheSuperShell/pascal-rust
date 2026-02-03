program fib;
    function fib(val: integer): integer;
    begin
        if val <= 2 then
            exit(val);
        exit(fib(val - 1) + fib(val - 2));
    end;
    var i: integer;
begin
    for i := 1 to 25 do
        writeln(fib(i))
end.