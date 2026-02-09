program n;
var arr: array[0..10] of real;
    dyn_arr: array of real;
    a: real;
begin
    a := arr['c'];
    a := dyn_arr['c'];
    a := a[0];
end.
