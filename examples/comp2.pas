program compiled;
    var res: integer = 0;

    procedure fib(out rs: integer; num: integer)
    var inter: integer = 5;
    begin
        if num <= 2 then
            rs := num
        else
            begin
                fib(rs, num - 1);
                inter := rs;
                fib(rs, num - 2);
                inter := inter + rs;
                rs := inter;
            end;
    end;
begin
    fib(res, 30);
    writeln(res);
end.