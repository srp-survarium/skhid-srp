package survarium.lobbypick
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.external.ExternalInterface;
   import flash.utils.getDefinitionByName;
   import survarium.data.LocalizationData;
   import survarium.rooms.CardWindow;
   import survarium.rooms.RoomsLook;

   // The lobby window (exe patch 81, docs/notes/lobby-scenes.md): four cards as the rooms' map vote
   // draws its own (CardWindow), the lobby's render over the whole card, its name on a dark band. A
   // click on one picks that lobby: srp is told (its 3B at PICK_BASE + id, the lobby's id), gives
   // the pick back in 0xA1, and the exe shows that lobby behind its fade. The pick has the gold frame.
   // What it shows the exe tells it (`state`, from root.srp_lobby_state).
   public class LobbyWindow extends CardWindow
   {
      public static const PICK_BASE:int = 2095000000;

      // the cards in id order, 1 to 4: their names (dictionary keys) and pictures (bitmaps/)
      private static const NAMES:Array = ["st_srp_lobby_room","st_srp_lobby_basement","st_srp_lobby_bunker","st_srp_lobby_hangar"];
      private static const PICTURES:Array = ["SrpLobbyRoom","SrpLobbyBasement","SrpLobbyBunker","SrpLobbyHangar"];

      // a vote card's size (RoomsRoom: a third of the rooms window less its margins, 190 high), two by two
      private static const CARD_W:int = 280;
      private static const CARD_H:int = 190;
      private static const BAND_H:int = 58;
      private static const NAME_SIZE:int = 21;

      // what the exe said: the pick (id)
      private var _pick:int = 1;
      // a click's pick, until the exe says it
      private var _asked:int = 0;

      public function LobbyWindow()
      {
         super(CARD_W,CARD_H,BAND_H,NAME_SIZE);
         name = "srpLobbyWindow";
         var pictures:Array = [];
         for each(var picture:String in PICTURES)
         {
            var cls:Class = getDefinitionByName(picture) as Class;
            pictures.push(new Bitmap(RoomsLook.cover(new cls() as BitmapData,CARD_W,CARD_H)));
         }
         build(pictures,2);
         LocalizationData.addCallback(this.labels,"st_srp_lobby_title");
         this.redraw();
      }

      private function labels(title:String) : void
      {
         setTitle(title.toUpperCase());
         for(var i:int = 0; i < NAMES.length; i++)
         {
            setLabel(i,LocalizationData.getName(NAMES[i]));
         }
      }

      // The exe's state (root.srp_lobby_state): the pick, the lobby shown.
      public function state(pick:uint, shown:uint) : void
      {
         this._pick = int(pick);
         if(this._asked == this._pick)
         {
            this._asked = 0;
         }
         this.redraw();
      }

      private function marked() : int
      {
         return this._asked != 0 ? this._asked : this._pick;
      }

      private function redraw() : void
      {
         mark(this.marked() - 1);
      }

      // A card pressed: its lobby picked (not the pick already), and the window closed.
      override protected function pressed(card:int) : void
      {
         var id:int = card + 1;
         if(id != this.marked())
         {
            this._asked = id;
            this.redraw();
            ExternalInterface.call("request_players_range",PICK_BASE + id,1);
         }
         close();
      }
   }
}
