// rougth message structure
{
    kind: "updatePainting" | "updateChat" | ..., // kind is what is inside message
    sender,
    ...args
}

// server sends to client
{
    kind: "updatePainting",
    sender,
    lines
}

{
    kind: "updateChat", // wrong guess
    sender
    message
}

{
    kind: "guessedByClient", // right guess
    sender,
    playerWhoGuessedIt
}

{
    kind: "lobbystate", // when player joins and when player leaves
    allplayers: { name, ready },
    timeTillAutomaticStart // client continues counting on its own
    
}

{
    kind: "gamestate",
    playersInGame: {name, (score,) turn }, //turn is boolean
    timeTillRoundEnds // client continues counting on its own
    word // only for drawer
    //  state: "choosing" | "guessing/drawing" 
    //lengthOfWord
    //chars: arrayOf(unknown | char)
}

{
    kind: "startRound",
    members,
    word,
    drawer
}

{
    kind: "newWord",
    word,
    drawer
}

{
    kind: "endRound",

}

// client sends to server

{
    kind: "guess",
    sender
    message
}