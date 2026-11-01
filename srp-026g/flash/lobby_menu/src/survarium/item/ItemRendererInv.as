package survarium.item
{
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import survarium.Util;
   import survarium.constants.Colors;
   import survarium.controls.FixedText;
   import survarium.controls.InventoryList;
   import survarium.controls.ItemRendererBase;
   import survarium.controls.LabelCost;
   import survarium.core.Icon;
   import survarium.data.ItemData;
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
      
      // A shop entry of an item the account owns (shop-owned-mark.md, the owner's of 2026-11-01): a
      // green check mark on a dark square at the top-right corner, where an owned tile shows its
      // equipped mark. The level (top-left) and the price (bottom) stay as they are.
      protected var _ownedMark:Shape;

      private static const OWNED_X:Number = 158;

      private static const OWNED_Y:Number = 6;

      private static const OWNED_SIZE:Number = 18;

      public function ItemRendererInv()
      {
         super();
      }

      protected function updateOwnedMark(show:Boolean) : void
      {
         if(!show)
         {
            if(this._ownedMark)
            {
               this._ownedMark.visible = false;
            }
            return;
         }
         if(!this._ownedMark)
         {
            this._ownedMark = new Shape();
            this._ownedMark.graphics.beginFill(0,0.6);
            this._ownedMark.graphics.drawRect(0,0,OWNED_SIZE,OWNED_SIZE);
            this._ownedMark.graphics.endFill();
            this._ownedMark.graphics.lineStyle(2.5,Colors.GREEN);
            this._ownedMark.graphics.moveTo(4,9.5);
            this._ownedMark.graphics.lineTo(7.5,13);
            this._ownedMark.graphics.lineTo(14,5);
            this._ownedMark.x = OWNED_X;
            this._ownedMark.y = OWNED_Y;
            addChild(this._ownedMark);
         }
         this._ownedMark.visible = true;
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
         if(this._ownedMark)
         {
            this._ownedMark.alpha = contentAlpha;
         }
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
            this.updateLoadoutBadge(item.itemId,true,this.textFieldCount,this.textFieldInfo);
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
            this.updateLoadoutBadge(item.itemId,isStack && !isShopItem,this.textFieldCount,this.textFieldInfo);
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
         // A bundle's id is not an item's: only the shop's items.
         this.updateOwnedMark(item.descriptor == ItemPlace.SHOP && InventoryList.owned(item.dictId));
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

