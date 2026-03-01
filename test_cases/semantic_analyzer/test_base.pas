program n;
    var i: integer;
begin
    i := 10 + 35;
    i := i { #one } * 2;
    i := 5 + ({ #paren } 3.14 { #two } / 2);
    writeln({ #three }i);
end.