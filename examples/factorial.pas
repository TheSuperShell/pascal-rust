program factorial;
    const pi = 3.14;
    type age = 0..100;
        some_values = (
            one,
            two,
            three
        );
    var my_age: age = 25;
    function some_func(a, b: integer; c: real): string;
    begin
        c := a + b;
        result := 'hello'
    end;
    var 2: string;
    var arr: array[0..100] of integer;
    a, b: boolean = true;
    i: integer;
begin
    my_age := 1000;
    writeln(my_age);
    for i := 0 to 100 do
        begin
            if i > 50  then
                begin
                    arr[i] := 50;
                    continue;
                end;
            arr[i] := i * i;
        end;
    writeln(arr);
    a := (5 > 10) = true and b;
    writeln(a);
    writeln(pi);
    b := a;
    c := some_func(1, 2, 3);
    writeln(c);
end.