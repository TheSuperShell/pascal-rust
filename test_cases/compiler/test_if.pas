program n;
    procedure if_elif_else(num: integer)
    begin
        if num < 0 then
            writeln(-1)
        else if num = 0 then
            writeln(0)
        else
            writeln(1)
    end;

    procedure if_elseif(num: integer)
    begin
        if num < 0 then
            writeln(-1)
        else if num = 0 then
            writeln(0)
    end;

    procedure if_else(num: integer)
    begin
        if num < 0 then
            writeln(-1)
        else
            writeln(1)
    end;

    procedure if_(num: integer)
    begin
        if num < 0 then
            writeln(-1)
    end;
begin
    if_elif_else(-10);
    if_elif_else(0);
    if_elif_else(13);

    if_else(-1);
    if_else(0);
    if_else(1);

    if_(-1);
    if_(0);
    if_(1);

    if_elseif(-1);
    if_elseif(0);
    if_elseif(1);
end.