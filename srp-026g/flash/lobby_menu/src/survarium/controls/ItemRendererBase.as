package survarium.controls
{
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import survarium.constants.Colors;
   import survarium.player.LocalPlayerProfile;
   
   public class ItemRendererBase extends ItemRendererCore
   {
      
      public var iconRenderer:IconRenderer;
      
      // The loadouts that use an owned item (presets.md; the owner's of
      // 2026-10-23), every one, the active one's too: their roman numerals
      // ("II", "I IV", "I-V") in a small badge at the bottom-left corner,
      // above the name, mirroring the stack count's corner: the count's
      // font and size, the equipped green, on a dark backing so it reads
      // over any icon. Drawn when the item is (the presets' list redraws the
      // inventory when the uses change). The inventory's tiles
      // (ItemRendererInv) and the craft window's slots (ItemRendererFull)
      // draw it; both tiles have the same corner.
      protected var _loadoutBadge:Sprite;
      
      protected var _loadoutText:TextField;
      
      private static const BADGE_X:Number = 6;
      
      private static const BADGE_BOTTOM:Number = 94;
      
      private static const BADGE_PAD:Number = 4;
      
      public function ItemRendererBase()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.iconRenderer.center = true;
      }
      
      override protected function cleanData() : void
      {
         this.iconRenderer.content = null;
      }
      
      override protected function updateData() : void
      {
         this.iconRenderer.content = _data.icon;
      }
      
      // `font`: the tile's field in the count's font and size (CalibriBold
      // 14); `info`: its debug id, at the same corner, which the badge goes
      // above when it shows.
      protected function updateLoadoutBadge(itemId:uint, owned:Boolean, font:TextField, info:TextField) : void
      {
         var mark:String = owned ? LocalPlayerProfile.loadoutMark(itemId) : "";
         if(mark == "")
         {
            if(this._loadoutBadge)
            {
               this._loadoutBadge.visible = false;
            }
            return;
         }
         if(!this._loadoutBadge)
         {
            var format:TextFormat = font.length > 0 ? font.getTextFormat() : font.defaultTextFormat;
            format.align = TextFormatAlign.LEFT;
            format.color = Colors.EQUIPPED;
            this._loadoutText = new TextField();
            this._loadoutText.defaultTextFormat = format;
            this._loadoutText.embedFonts = true;
            this._loadoutText.selectable = false;
            this._loadoutText.mouseEnabled = false;
            this._loadoutText.multiline = false;
            this._loadoutText.wordWrap = false;
            this._loadoutText.autoSize = TextFieldAutoSize.LEFT;
            this._loadoutBadge = new Sprite();
            this._loadoutBadge.mouseEnabled = false;
            this._loadoutBadge.mouseChildren = false;
            this._loadoutBadge.addChild(this._loadoutText);
            addChild(this._loadoutBadge);
         }
         this._loadoutText.text = mark;
         var w:Number = Math.ceil(this._loadoutText.textWidth) + 2 * BADGE_PAD;
         var h:Number = Math.ceil(this._loadoutText.textHeight);
         this._loadoutText.x = BADGE_PAD - 2;
         this._loadoutText.y = -2;
         this._loadoutBadge.graphics.clear();
         this._loadoutBadge.graphics.beginFill(0,0.6);
         this._loadoutBadge.graphics.drawRect(0,0,w,h);
         this._loadoutBadge.graphics.endFill();
         this._loadoutBadge.x = BADGE_X;
         // Above the debug id (the same corner) when it shows.
         this._loadoutBadge.y = (info.visible ? info.y : BADGE_BOTTOM) - h;
         this._loadoutBadge.visible = true;
      }
   }
}

