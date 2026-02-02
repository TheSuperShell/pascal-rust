program factorial;
    const pi = 3.14;
    type age = integer;
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
    a, b: boolean = true;
begin
    a := (5 > 10) = true and b;
    writeln(a);
    writeln(pi);
    b := a;
    c := some_func(1, 2, 3);
    writeln(c);
    readln(c);
    writeln(c);
end.