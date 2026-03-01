program n;
    var arr: array[-5..0] of int64;
        i: integer;
begin
    arr[-3] := 10;
    writeln(arr[-3]);
    for i := -5 to 0 do
        arr[i] := i + 5;
    for i := -5 to 0 do
        writeln(arr[i]);
end.