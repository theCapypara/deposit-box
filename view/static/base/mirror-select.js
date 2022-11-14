document.querySelector('.mirror-select').style.display = "block";

var mirorSelect = document.getElementById('mirror-select');
mirorSelect.onchange = function() {
    var selected = mirorSelect.value.toLowerCase();
    var dataAttribName = "data-href-" + selected;
    var dataAttribNameDataset = "href" + selected.charAt(0).toUpperCase() + selected.slice(1);
    document.querySelectorAll("[" + dataAttribName + "]").forEach(function (el) {
        el.href = el.dataset[dataAttribNameDataset];
    });
}
