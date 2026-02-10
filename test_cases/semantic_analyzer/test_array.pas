program n;
type age = 0..100;
    class_ages = array[age] of integer;
    class_ages_dyn = array of integer;
var class_a: class_ages;
    class_b: class_ages_dyn;
    a: age = 5;
begin
    class_a[ { #arr } a { #range }] := 35;
    class_b[ { #dyn_arr } a] := 10;
end.