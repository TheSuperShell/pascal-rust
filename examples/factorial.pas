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
begin
    a := (5 > 10) = true and b;
    b := a;
    c := some_func('text' = 'other_text');
    for a := 0 to 35 do
        b := 10;
    if a > b then
        a := b
    else if a < b then
        a(b)
    else
        b := a;
    while a <> b do
        begin
            a := true;
            b := false;
        end;
    a := not b[1];
end.