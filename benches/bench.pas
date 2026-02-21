program compiled;
    var b: integer = 5;

    function fib(num: integer): integer;
    begin
        if num = 1 then
            result := 1
        else if num = 2 then
            result := 2
        else
            result := fib(num - 1)  + fib(num - 2)
    end;
begin
    for b := 1 to 20 do
        writeln(fib(b))
end.