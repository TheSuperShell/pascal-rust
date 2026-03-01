
program n;
function fib(num: integer): integer;
begin
    if num <= 2 then
        fib{#res1} := num
    else
        result{#res2} := fib(num - 1) + fib(num - 2)
end;
var num: integer;
begin
    num := fib({#func}100);
    writeln(num)
end.