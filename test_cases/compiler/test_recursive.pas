program n;
function factorial(num: integer):integer;
begin
    if num <= 1 then
        exit(1);
    exit(factorial(num - 1) * num);
end;
begin
    writeln(factorial(5));
end.