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
        exit('value')
    end;
    var c: string;
    a, b: boolean;
    i: integer;
begin
    a := (5 > 10) = true and b;
    b := a;
    c := some_func(1, 2, 3);
    for i := 0 to 35 do
        b := a;
    if 10 > 34.5 then
        a := b
    else if 10 < 353 then
        b := true
    else
        b := a;
    while a <> b do
        begin
            a := true;
            b := false;
        end;
end.