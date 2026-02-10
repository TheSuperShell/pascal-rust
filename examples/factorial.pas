program fib;
type days= (mon, tue, wen, thu, fri);
    week= mon..fri;
    function fib(val: integer): integer;
    begin
        if val <= 2 then
            exit(val);
        exit(fib(val - 1) + fib(val - 2));
    end;
    var i: integer;
    var arr: array[mon..fri] of days;
    var day: week;
begin
    for i := 0 to 25 do
        writeln('fib(', i, ') = ', fib(i));
    for day := mon to fri do
        arr[day] := day;
    writeln(arr);
end.