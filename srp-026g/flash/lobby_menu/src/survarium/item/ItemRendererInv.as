package survarium.item
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import survarium.Util;
   import survarium.constants.Colors;
   import survarium.controls.FixedText;
   import survarium.controls.ItemRendererBase;
   import survarium.controls.LabelCost;
   import survarium.core.Icon;
   import survarium.data.ItemData;
   import survarium.player.LocalPlayerProfile;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.ItemCondition;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemRank;
   import survarium.constants.MoneyType;
   import survarium.constants.RendererState;
   import survarium.constants.SuperCategory;
   
   public class ItemRendererInv extends ItemRendererBase
   {
      
      public var textName:FixedText;
      
      public var brokenIcon:MovieClip;
      
      protected var _scopeBadge:Icon;
      
      public var paintedIcon:MovieClip;
      
      public var textLevel:TextField;
      
      public var textFieldInfo:TextField;
      
      public var textFieldCount:TextField;
      
      public var sellerIcon:MovieClip;
      
      public var labelCost:LabelCost;
      
      public var damageIndicator:MovieClip;
      
      public var bundleBackground:MovieClip;
      
      public var bottomBackground:MovieClip;
      
      public var equippedSprite:MovieClip;
      
      public var equippedBorder:MovieClip;
      
      // The loadouts that use an owned item (presets.md; the owner's of
      // 2026-10-23), every one, the active one's too: their roman numerals
      // ("II", "I IV", "I-V") in a small badge at the bottom-left corner,
      // above the name, mirroring the stack count's corner: the count's
      // font and size, the equipped green, on a dark backing so it reads
      // over any icon. Drawn when the item is (the presets' list redraws the
      // inventory when the uses change).
      protected var _loadoutBadge:Sprite;
      
      protected var _loadoutText:TextField;
      
      private static const BADGE_X:Number = 6;
      
      private static const BADGE_BOTTOM:Number = 94;
      
      private static const BADGE_PAD:Number = 4;
      
      public function ItemRendererInv()
      {
         super();
      }
      
      protected function updateScopeBadge(item:ItemData, show:Boolean) : void
      {
         var scope:Object = null;
         if(Boolean(this._scopeBadge) && Boolean(this._scopeBadge.parent))
         {
            this._scopeBadge.parent.removeChild(this._scopeBadge);
         }
         this._scopeBadge = null;
         if(!show || !item.isWeapon)
         {
            return;
         }
         scope = UpgradeWindow.attachedScope(item);
         if(!scope || !scope.icon)
         {
            return;
         }
         this._scopeBadge = (scope.icon as Icon).clone();
         this._scopeBadge.scaleX = 0.4;
         this._scopeBadge.scaleY = 0.4;
         this._scopeBadge.x = this.paintedIcon.x + this.paintedIcon.width - this._scopeBadge.width;
         this._scopeBadge.y = this.paintedIcon.y + this.paintedIcon.height - this._scopeBadge.height;
         this._scopeBadge.mouseEnabled = false;
         this._scopeBadge.mouseChildren = false;
         addChild(this._scopeBadge);
         this.paintedIcon.visible = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function draw() : void
      {
         super.draw();
      }
      
      override public function updateState(itemState:uint = RendererState.ACTIVE) : void
      {
         var contentAlpha:Number = itemState == RendererState.ACTIVE ? 1 : 0.6;
         if(iconRenderer)
         {
            iconRenderer.alpha = itemState == RendererState.LOCKED ? 0.6 : 1;
         }
         this.textName.alpha = contentAlpha;
         this.brokenIcon.alpha = contentAlpha;
         this.paintedIcon.alpha = contentAlpha;
         this.textLevel.alpha = contentAlpha;
         this.textFieldInfo.alpha = contentAlpha;
         this.textFieldCount.alpha = contentAlpha;
         this.sellerIcon.alpha = contentAlpha;
         this.labelCost.alpha = contentAlpha;
         this.damageIndicator.alpha = contentAlpha;
         this.equippedSprite.alpha = contentAlpha;
         this.equippedBorder.alpha = contentAlpha;
         if(this._loadoutBadge)
         {
            this._loadoutBadge.alpha = contentAlpha;
         }
      }
      
      protected function updateLoadoutBadge(item:ItemData, owned:Boolean) : void
      {
         var mark:String = owned ? LocalPlayerProfile.loadoutMark(item.itemId) : "";
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
            var count:TextField = this.textFieldCount;
            var format:TextFormat = count.length > 0 ? count.getTextFormat() : count.defaultTextFormat;
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
         this._loadoutBadge.y = (this.textFieldInfo.visible ? this.textFieldInfo.y : BADGE_BOTTOM) - h;
         this._loadoutBadge.visible = true;
      }
      
      override protected function updateData() : void
      {
         var nameColor:uint = 0;
         var condition:int = 0;
         var trackColor:uint = 0;
         var colorTransform:ColorTransform = null;
         var isShopRow:Boolean = false;
         var item:ItemData = _data as ItemData;
         super.updateData();
         var isStack:Boolean = item.isStack;
         var isShopItem:Boolean = item.descriptor == ItemPlace.SHOP || item.descriptor == ItemPlace.BUNDLE;
         var isOwnedItem:Boolean = !isStack && !isShopItem;
         this.bundleBackground.visible = item.fromBundle;
         this.textName.text = item.name;
         if(Boolean(item.rank))
         {
            if(item.rank == ItemRank.ONE)
            {
               nameColor = Colors.RANK_1;
            }
            else if(_data.rank == ItemRank.TWO)
            {
               nameColor = Colors.RANK_2;
            }
            else
            {
               nameColor = Colors.RANK_3;
            }
         }
         else
         {
            nameColor = Colors.LIGHT_GRAY;
         }
         // Skhid: a golden item's name is gold, whatever its rank (golden_weapons.md, premium_armour.md)
         if(ItemFactory.isGolden(item.dictId))
         {
            nameColor = Colors.GOLD;
         }
         this.textName.textField.textColor = nameColor;
         this.textLevel.text = item.itemLevel.toString();
         this.textLevel.visible = item.itemLevel > 0 && item.superCategory != SuperCategory.UPGRADES;
         this.sellerIcon.visible = Boolean(item.sellerIcon);
         if(Boolean(this.sellerIcon.numChildren))
         {
            this.sellerIcon.removeChildAt(0);
         }
         if(Boolean(item.sellerIcon))
         {
            this.sellerIcon.addChild(item.sellerIcon);
         }
         this.textLevel.x = Boolean(this.sellerIcon.numChildren) ? 24 : this.sellerIcon.x;
         trace("IRV::updateData",item.name,item.isStack,item.value,"shop:",isShopItem,this.equippedSprite,this.equippedBorder,"|",item.itemLevel,this.textLevel.text);
         if(isOwnedItem)
         {
            this.textFieldInfo.visible = MainScene.showDebugInfo;
            if(this.textFieldInfo.visible)
            {
               this.textFieldInfo.text = item.itemId.toString();
            }
            this.equippedSprite.visible = item.equipped;
            this.equippedBorder.visible = item.equipped;
            this.brokenIcon.visible = item.broken;
            if(this.brokenIcon.visible)
            {
               this.brokenIcon.x = this.equippedSprite.visible ? 145 : 160;
            }
            this.paintedIcon.visible = Boolean(item.upgrades) && item.upgrades.length > 0;
            this.updateScopeBadge(item,true);
            this.updateLoadoutBadge(item,true);
            this.damageIndicator.visible = isOwnedItem && item.value < ItemCondition.FULL;
            this.damageIndicator.track.scaleX = _data.value / ItemCondition.FULL;
            condition = _data.value / ItemCondition.FULL * 510;
            trackColor = Util.getColor(Math.min(510 - condition,255),Math.min(condition,255),0);
            colorTransform = new ColorTransform();
            colorTransform.color = trackColor;
            this.damageIndicator.track.transform.colorTransform = colorTransform;
            this.damageIndicator.track.alpha = 0.4;
         }
         else
         {
            this.textFieldInfo.visible = false;
            this.equippedSprite.visible = false;
            this.equippedBorder.visible = false;
            this.brokenIcon.visible = false;
            this.paintedIcon.visible = false;
            this.updateScopeBadge(item,false);
            this.updateLoadoutBadge(item,isStack && !isShopItem);
            this.damageIndicator.visible = false;
         }
         if(isShopItem)
         {
            this.labelCost.visible = true;
            this.labelCost.moneyType = item.isPremium ? MoneyType.PREMIUM : MoneyType.REGULAR;
            this.labelCost.textField.textColor = item.isPremium ? Colors.GOLD : Colors.PRICE_REG;
            this.labelCost.cost = item.cost;
         }
         else
         {
            this.labelCost.visible = false;
         }
         if(isStack && !isShopItem)
         {
            this.textFieldCount.visible = true;
            this.textFieldCount.text = "x" + item.value;
         }
         else
         {
            this.textFieldCount.visible = false;
         }
         if(Boolean(this.bottomBackground))
         {
            isShopRow = _data.descriptor == ItemPlace.SHOP || _data.descriptor == ItemPlace.BUNDLE;
            this.bottomBackground.height = isShopRow ? 54 : 34;
            this.bottomBackground.y = isShopRow ? 75 : 95;
         }
      }
   }
}

