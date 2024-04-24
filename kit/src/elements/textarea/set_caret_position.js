var e = document.getElementById("$ELEMENT");
var pos = parseInt("$POS");
if(e != null) {
    if (e.markdownEditor) {       
        console.log(e.markdownEditor.value()) 
        e.markdownEditor.codemirror.dispatch({
            selection: {
              anchor: pos,
              head: pos,
            },
          });
    } else {
        e.focus();
        e.setSelectionRange(pos, pos);
    }
}