function ih(el, dt) {
	function escapeHtml(unsafe) {
		return unsafe
			 .replace(/&/g, "&amp;")
			 .replace(/</g, "&lt;")
			 .replace(/>/g, "&gt;")
			 .replace(/"/g, "&quot;")
			 .replace(/'/g, "&#039;");
	}
	el.innerHTML = escapeHtml(dt);
}

window.addEventListener('load', function() {
	console.log("onload");

	var wsUri = (window.location.protocol=='https:' && 'wss://' || 'ws://') + window.location.host + '/ws/';
	var conn = new WebSocket(wsUri);
	conn.onopen = function() {
		console.log('Connected.');
		conn.send(JSON.stringify({type: "refresh"}));
	};

	conn.onmessage = function(e) {
		console.log('Received: ' + e.data);
		var obj = JSON.parse(e.data);
		switch (obj.type) {
			case "text": {
				sentence = obj.text;
				resetRacer();
				/*
				timer = null;
				validEntered = "";
				ih(preLeft, sentence);
				*/
				break;
			}
		}
	};
	conn.onclose = function() {
		console.log('Disconnected.');
	};

	function wsReady(ws) {
		return ws.readyState === WebSocket.OPEN;
	}


	document.getElementById("ctrl_refresh").addEventListener("click", function() {
		console.log("refresh clicked");
		if (wsReady(conn)) {
			conn.send(JSON.stringify({type: "refresh"}));
		}
	});

	var sentence = null;
	var validEntered = null;
	var timer = null;


	var infoSpeed = document.getElementById("info_speed");
	setInterval(function() {
		if (validEntered == null || timer == null) {
			return;
		}

		var delta = (Date.now() - timer) / 1000.0;
		var wpm = Math.round((validEntered.length/5.0) * (60.0/delta));

		ih(infoSpeed, wpm + " WPM");
	}, 500);

	//Preview
	var preValid = document.getElementById("pre-v");

	var preCurrentValid = document.getElementById("pre-cv");
	var preCurrentInvalid = document.getElementById("pre-ci");
	var preInvalid = document.getElementById("pre-i");
	var preCurrent = document.getElementById("pre-c");

	var preLeft = document.getElementById("pre-l");

	var textInputBox = document.getElementById("text-input");
	
	function resetRacer() {
		validEntered = "";
		lastValidEntered = "";
		timer = null;

		ih(preLeft, sentence);
		ih(preValid, "");
		ih(preCurrentValid, "");
		ih(preCurrentInvalid, "");
		ih(preInvalid, "");
		ih(preCurrent, "");

		textInputBox.value = "";
	}

	textInputBox.addEventListener('input', function(change) {
		if (sentence == null || validEntered == null) {
			return;
		}

		if (timer == null) {
			timer = Date.now();
		}
		var delta = Date.now() - timer;

		if (wsReady(conn)) {
			conn.send(JSON.stringify({type: "change", data: change.data, change: change.inputType, ts: delta}));
		}

		var currentInput = textInputBox.value;

		var evalCV = "";
		var evalCI = "";
		var evalC = "";
		var mistake = false;
		for (var i = 0; i < currentInput.length; i++) {
			if (currentInput[i] === sentence[validEntered.length + i]) {
				//console.log(currentInput[i] + " - OK");
				if (!mistake) {
					evalCV += currentInput[i];
				} else {
					evalCI += currentInput[i];
				}
			} else {
				//console.log(currentInput[i] + " != " + sentence[validEntered + i]);
				mistake = true;
				evalCI += currentInput[i];
			}
		}



		var remainingWords = sentence.substring(validEntered.length);
		var remainingWordsSplit = remainingWords.split(" ");
		var currentWord = remainingWordsSplit[0];

		//var currentWord = remainingWordsSplit[0];
		//var remainingExceptCurrent = remainingWordsSplit.slice(1).join(" ");

		if (!mistake && evalCI === "" && change.data === " ") {
			//Space pressed with 0 mistakes, jump to next word
			validEntered += currentInput;

			//console.log(remainingWordsSplit[1]);

			ih(preCurrentValid, "");
			ih(preCurrentInvalid, "");
			ih(preCurrent, remainingWordsSplit[1]);
			ih(preLeft, sentence.substring(validEntered.length + remainingWordsSplit[1].length));
			textInputBox.value = "";
		} else {
			//console.log("cv: " + evalCV);
			//console.log("ci: " + remainingWords.substring(evalCV.length, evalCV.length + evalCI.length));

			var combinedEvalLength = evalCV.length + evalCI.length;
			ih(preCurrentValid, evalCV); //This can be read out from the string, because it has been verified

			var currentSubbedWord = currentWord.substring(combinedEvalLength);
			//console.log(remainingWords.substring(evalCV.length, combinedEvalLength).length - currentSubbedWord.length);
			//console.log(remainingWords.substring(evalCV.length, combinedEvalLength).length);
			//console.log(currentSubbedWord.length);
			//console.log(currentSubbedWord);

			//ih(preCurrentInvalid, currentSubbedWord);
			var currentInvalid = currentWord.substring(evalCV.length, combinedEvalLength);
			ih(preCurrentInvalid, currentInvalid);

			if (evalCI.length + evalCV.length > currentWord.length) {
				var tst = remainingWords.substring(currentWord.length, combinedEvalLength);
				//console.log("tst: " + tst);
				//console.log("combevallen: " + combinedEvalLength);
				ih(preInvalid, tst);
				ih(preCurrent, "");
			} else {
				ih(preInvalid, "");
				ih(preCurrent, currentSubbedWord);
			}

			ih(preLeft, remainingWords.substring(combinedEvalLength + currentSubbedWord.length));
		}
		ih(preValid, validEntered);

		//console.log(validEntered + currentInput);
		if ((validEntered + currentInput) == sentence) {
			console.log("done!");
			if (wsReady(conn)) {
				conn.send(JSON.stringify({type: "done", ts: delta}));
			}
			location.reload();
		}
	});

	textInputBox.value = "";

});

