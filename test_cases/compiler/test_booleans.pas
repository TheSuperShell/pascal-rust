program n;
    type age = integer;
    const drinking_age = 21;
    var is_legal: boolean = true;
        his_age: age = 15;
begin
    is_legal := his_age >= drinking_age;
    writeln(is_legal);
    is_legal := (his_age - drinking_age) < 0;
    writeln(is_legal);
end.