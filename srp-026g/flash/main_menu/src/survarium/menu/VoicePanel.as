package survarium.menu
{
   import flash.display.Graphics;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.core.UIComponent;

   // Voice chat's mutes, in the Esc menu of a match (docs/notes/voice-chat.md): the teammates
   // (voice is team-only: no enemy is listed), a checkbox each that mutes that player, and one
   // that mutes them all. The Esc menu frees the cursor, in Blackout too; the Tab list does not.
   // Exe patch 66 fills it each time the menu opens in a match (MainMenu.srp_voice_panel) and
   // empties it in the lobby; the mutes are the exe's (per account, kept across matches): a
   // click tells it (srp_voice_mute, srp_voice_mute_all) and it plays nothing of a muted player.
   public class VoicePanel extends UIComponent
   {

      protected static const WIDTH:Number = 260;

      protected static const PADDING:Number = 14;

      protected static const ROW:Number = 34;

      protected var _title:TextField;

      protected var _all:CheckBoxSkinned;

      // Each teammate's checkbox, and its slot (the exe's key for it).
      protected var _rows:Array = [];

      protected var _slots:Array = [];

      // Set while the boxes are set from the exe's data: no click to report.
      protected var _filling:Boolean;

      // style: the Esc menu's own title field, whose font the panel's title takes.
      public function VoicePanel(style:TextField)
      {
         super();
         _title = new TextField();
         if(style)
         {
            this._title.defaultTextFormat = style.getTextFormat();
            this._title.embedFonts = style.embedFonts;
         }
         this._title.autoSize = TextFieldAutoSize.LEFT;
         this._title.selectable = false;
         this._title.mouseEnabled = false;
         this._title.x = PADDING;
         this._title.y = PADDING - 4;
         addChild(this._title);
         _all = this.makeBox();
         this._all.addEventListener(Event.SELECT,this.onAll);
         visible = false;
      }

      // title, the "mute all" box's label, then for each teammate: its slot, its name, muted.
      // No teammates: hidden.
      public function setData(title:String, allLabel:String, rows:Array) : void
      {
         var count:uint = uint(rows.length / 3);
         _filling = true;
         this._title.text = title;
         this._all.label = allLabel;
         while(this._rows.length < count)
         {
            this._rows.push(this.makeBox());
            this._rows[this._rows.length - 1].addEventListener(Event.SELECT,this.onRow);
         }
         _slots = [];
         var top:Number = this._title.y + this._title.height + 8;
         var everyone:Boolean = count > 0;
         var i:uint = 0;
         var box:CheckBoxSkinned;
         while(i < this._rows.length)
         {
            box = this._rows[i];
            box.visible = i < count;
            if(i < count)
            {
               this._slots.push(uint(rows[3 * i]));
               box.label = String(rows[3 * i + 1]);
               box.selected = Boolean(rows[3 * i + 2]);
               everyone = everyone && box.selected;
               box.y = top + i * ROW;
            }
            i++;
         }
         this._all.selected = everyone;
         this._all.y = top + count * ROW + 6;
         _filling = false;
         this.drawBackground(this._all.y + ROW + PADDING - 6);
         visible = count > 0;
      }

      protected function makeBox() : CheckBoxSkinned
      {
         var box:CheckBoxSkinned = new CheckBoxSkinned();
         box.autoSize = "none";
         box.setSize(WIDTH - 2 * PADDING,30);
         box.x = PADDING;
         box.enabled = true;
         // Scaleform leaves a component made by a script fully transparent.
         box.alpha = 1;
         addChild(box);
         return box;
      }

      protected function drawBackground(height:Number) : void
      {
         var g:Graphics = graphics;
         g.clear();
         g.lineStyle(1,0x5B6261,1);
         g.beginFill(0,0.7);
         g.drawRect(0,0,WIDTH,height);
         g.endFill();
      }

      protected function onRow(event:Event) : void
      {
         if(this._filling)
         {
            return;
         }
         var index:int = this._rows.indexOf(event.target);
         if(index < 0 || index >= this._slots.length)
         {
            return;
         }
         var box:CheckBoxSkinned = this._rows[index];
         ExternalInterface.call("srp_voice_mute",this._slots[index],box.selected);
         this.syncAll();
      }

      protected function onAll(event:Event) : void
      {
         if(this._filling)
         {
            return;
         }
         var muted:Boolean = this._all.selected;
         ExternalInterface.call("srp_voice_mute_all",muted);
         _filling = true;
         var i:uint = 0;
         while(i < this._slots.length)
         {
            CheckBoxSkinned(this._rows[i]).selected = muted;
            i++;
         }
         _filling = false;
      }

      // "Mute all" is checked while every teammate is muted.
      protected function syncAll() : void
      {
         var everyone:Boolean = this._slots.length > 0;
         var i:uint = 0;
         while(i < this._slots.length)
         {
            everyone = everyone && CheckBoxSkinned(this._rows[i]).selected;
            i++;
         }
         _filling = true;
         this._all.selected = everyone;
         _filling = false;
      }
   }
}
