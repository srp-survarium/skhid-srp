package survarium.modes
{
   import flash.display.Graphics;
   import flash.filters.GlowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.controls.ResearchUnits;

   // Artifact Hunt's one look, shared by its row under the bars, the minimaps and the Tab list: the
   // teams' colours of Research (own blue, other red, nobody grey) and the artefact, a glowing orb
   // in the pale gold of its light.
   public class ArtefactHuntLook
   {
      public static const OWN:uint = ResearchUnits.OWN;

      public static const ENEMY:uint = ResearchUnits.ENEMY;

      public static const NOBODY:uint = ResearchUnits.NOBODY;

      // The artefact's light ("Glow of the Forest").
      public static const GLOW:uint = 0xF4E6A2;

      // Sudden death's caption: the hud's warning orange.
      public static const SUDDEN:uint = 0xFFB52E;

      public static function teamColour(team:int, state:ArtefactHuntState) : uint
      {
         return team < 0 ? NOBODY : (state.isOwn(team) ? OWN : ENEMY);
      }

      // The orb: a core and its halo, centred on 0, 0, `radius` its halo's. Bright: the artefact
      // itself; dim: none in the world.
      public static function drawOrb(g:Graphics, radius:Number, bright:Boolean) : void
      {
         g.lineStyle(Math.max(1,radius * 0.16),GLOW,bright ? 0.85 : 0.35);
         g.drawCircle(0,0,radius);
         g.lineStyle();
         g.beginFill(GLOW,bright ? 1 : 0.4);
         g.drawCircle(0,0,radius * 0.56);
         g.endFill();
         g.beginFill(0xFFFFFF,bright ? 0.9 : 0.3);
         g.drawCircle(-radius * 0.18,-radius * 0.18,radius * 0.2);
         g.endFill();
      }

      // The light round what holds the artefact (a minimap's dot, the carrier's ring).
      public static function glow(strength:Number = 2) : GlowFilter
      {
         return new GlowFilter(GLOW,0.9,6,6,strength);
      }

      // A text of the row: `source`'s embedded font (a label of the movie, never shown), white with a
      // dark edge, centred, one line.
      public static function text(source:TextField, size:Number, colour:uint = 0xFFFFFF) : TextField
      {
         var format:TextFormat = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
         var field:TextField = new TextField();
         format.size = size;
         format.color = colour;
         format.align = "center";
         field.defaultTextFormat = format;
         field.embedFonts = source.embedFonts;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = false;
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.CENTER;
         field.filters = [new GlowFilter(0,0.9,3,3,3)];
         field.visible = false;
         return field;
      }

      // "MM:SS" of ms, the seconds rounded up: the exe's own countdowns' form ("%02d:%02d").
      public static function clock(ms:Number) : String
      {
         var seconds:int = Math.ceil(Math.max(0,ms) / 1000);
         var minutes:int = int(seconds / 60);
         seconds %= 60;
         return (minutes < 10 ? "0" : "") + minutes + ":" + (seconds < 10 ? "0" : "") + seconds;
      }
   }
}
