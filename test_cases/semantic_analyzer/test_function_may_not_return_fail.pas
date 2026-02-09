program n;
function f1: real;
Begin
    if true then
        exit(10.0);
end;
function f2: integer ;
var i: integer;
begin
    for i := 0 to 10 do
        exit(i);
end;
begin
end.