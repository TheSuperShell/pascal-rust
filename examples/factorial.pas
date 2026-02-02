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
    var c: string;
    var arr: array[0..100] of integer;
    a, b: boolean = true;
begin
    my_age := 1000;
    writeln(my_age);
    arr[1] := 10;
    writeln(arr[1]);
    a := (5 > 10) = true and b;
    writeln(a);
    writeln(pi);
    b := a;
    c := some_func(1, 2, 3);
    writeln(c);
end.