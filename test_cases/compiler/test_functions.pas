program n;
function func1(a, b, c, d: integer):integer;
begin
    exit(-(a + b) * c / d)
end;

function func2(a: integer):integer;
begin
    result := a * a
end;
begin
    writeln(func1(2, 3, 4, 2));
    writeln(func2(5));
end.