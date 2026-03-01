program n;
    var i: integer;
begin
    for i := -5 to 5 do
        begin
            if i >= 0 then
                break;
            if i = -3 then
                continue;
            writeln(i);
        end;
    i := 5;
    while i > 0 do
        begin
            writeln(i);
            i := i - 1;
        end;
end.