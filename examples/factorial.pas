program fib;
    function fib(val: integer): integer;
    begin
        if val <= 2 then
            exit(val);
        exit(fib(val - 1) + fib(val - 2));
    end;
    var i: char = 'a';
    var arr: array['a'..'z'] of char;
begin
    for i := 'a' to 'z' do
        arr[i] := i;
    writeln(arr)
end.