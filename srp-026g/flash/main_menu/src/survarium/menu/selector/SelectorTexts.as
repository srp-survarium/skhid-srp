package survarium.menu.selector
{
   // The selector's own texts, the exe's answer to ExternalInterface.call("srp_controls_texts")
   // (root.srp_controls_texts, exe patch 83: hud_options.rs's CONTROLS_TEXTS), in its order. A movie
   // holds no language text: before the answer every text is empty.
   public class SelectorTexts
   {

      public static const COUNT:int = 16;

      public var title:String;

      public var search:String;

      public var unbind:String;

      public var free:String;

      // "%1$s moves here from %2$s.": the action, its key now
      public var moves:String;

      // "%1$s will have no key.": the actions the key leaves
      public var loses:String;

      public var noMatch:String;

      public var choose:String;

      public var assign:String;

      public var cancel:String;

      public var lobby:String;

      // SrpKeysWindow.KINDS' names, by kind (the Key bindings window's own: st_srp_keys_*)
      public var groups:Array;

      public function SelectorTexts(values:Array)
      {
         super();
         var text:Function = function(index:int):String
         {
            return values != null && index < values.length && values[index] != null ? String(values[index]) : "";
         };
         this.title = text(0);
         this.search = text(1);
         this.unbind = text(2);
         this.free = text(3);
         this.moves = text(4);
         this.loses = text(5);
         this.noMatch = text(6);
         this.choose = text(7);
         this.assign = text(8);
         this.cancel = text(9);
         this.lobby = text(10);
         this.groups = [text(11),text(12),text(13),text(14),text(15)];
      }

      public static function format(text:String, ... values) : String
      {
         for(var i:int = 0; i < values.length; i++)
         {
            text = text.split("%" + (i + 1) + "$s").join(values[i]);
         }
         return text;
      }
   }
}
