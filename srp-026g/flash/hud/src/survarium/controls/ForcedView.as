package survarium.controls
{
   // What a forced interface shows (Blackout, exe patch 65; docs/notes/blackout.md): the parts it keeps
   // now, by its states (forced, Tab held, the chat typed in), in a KeptLayer over the hidden root. The
   // hud gives it the parts: always while forced (whenForced), while Tab is held (onTab), while the chat
   // is typed in (whileTyping); and the chat's history to hold on while Tab is held and the chat is not
   // typed in (history(true), and history(false) when no more).
   public class ForcedView
   {
      private var _layer:KeptLayer;

      private var _whenForced:Function;

      private var _onTab:Function;

      private var _whileTyping:Function;

      private var _history:Function;

      private var _forced:Boolean;

      private var _tab:Boolean;

      private var _typing:Boolean;

      private var _historyShown:Boolean;

      // What is kept now: given back as it was when no more kept.
      private var _kept:Array = [];

      public function ForcedView(layer:KeptLayer, whenForced:Function, onTab:Function, whileTyping:Function,
                                 history:Function)
      {
         super();
         this._layer = layer;
         this._whenForced = whenForced;
         this._onTab = onTab;
         this._whileTyping = whileTyping;
         this._history = history;
      }

      public function get forced() : Boolean
      {
         return this._forced;
      }

      public function set forced(value:Boolean) : void
      {
         this._forced = value;
         this.refresh();
      }

      public function set tab(value:Boolean) : void
      {
         this._tab = value;
         this.refresh();
      }

      public function set typing(value:Boolean) : void
      {
         this._typing = value;
         this.refresh();
      }

      // The parts kept now: none when not forced.
      public function parts() : Array
      {
         if(!this._forced)
         {
            return [];
         }
         var parts:Array = this._whenForced();
         if(this._tab)
         {
            parts = parts.concat(this._onTab());
         }
         if(this._typing)
         {
            parts = parts.concat(this._whileTyping());
         }
         return parts.filter(function(part:*, i:int, all:Array):Boolean
         {
            return part != null && all.indexOf(part) == i;
         });
      }

      // At every change of a state or of the parts available (death/respawn): what parts() says kept,
      // what was kept and is not now given back,
      // the chat's history held on for Tab (while typed in, the chat holds its lines itself).
      public function refresh() : void
      {
         var now:Array = this.parts();
         this._layer.keep(this._kept.filter(function(part:*, i:int, all:Array):Boolean
         {
            return now.indexOf(part) == -1;
         }),false);
         this._layer.keep(now,true);
         this._kept = now;
         var history:Boolean = this._forced && this._tab && !this._typing;
         if(history != this._historyShown)
         {
            this._historyShown = history;
            this._history(history);
         }
      }
   }
}
