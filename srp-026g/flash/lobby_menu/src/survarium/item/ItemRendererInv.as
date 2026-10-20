package survarium.item
{
   import flash.display.MovieClip;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import survarium.Util;
   import survarium.constants.Category;
   import survarium.constants.Colors;
   import survarium.controls.FixedText;
   import survarium.controls.ItemRendererBase;
   import survarium.controls.LabelCost;
   import survarium.core.Icon;
   import survarium.data.ItemData;
   import survarium.upgrade.UpgradeWindow;
   
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
      
      public function ItemRendererInv()
      {
         super();
      }
      
      protected function updateScopeBadge(param1:ItemData, param2:Boolean) : void
      {
         var _loc3_:Object = null;
         if(Boolean(this._scopeBadge) && Boolean(this._scopeBadge.parent))
         {
            this._scopeBadge.parent.removeChild(this._scopeBadge);
         }
         this._scopeBadge = null;
         if(!param2 || !param1.isWeapon)
         {
            return;
         }
         _loc3_ = UpgradeWindow.attachedScope(param1);
         if(!_loc3_ || !_loc3_.icon)
         {
            return;
         }
         this._scopeBadge = (_loc3_.icon as Icon).clone();
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
      
      override public function updateState(param1:uint = 0) : void
      {
         var _loc2_:Number = param1 == 0 ? 1 : 0.6;
         if(iconRenderer)
         {
            iconRenderer.alpha = param1 == 2 ? 0.6 : 1;
         }
         this.textName.alpha = _loc2_;
         this.brokenIcon.alpha = _loc2_;
         this.paintedIcon.alpha = _loc2_;
         this.textLevel.alpha = _loc2_;
         this.textFieldInfo.alpha = _loc2_;
         this.textFieldCount.alpha = _loc2_;
         this.sellerIcon.alpha = _loc2_;
         this.labelCost.alpha = _loc2_;
         this.damageIndicator.alpha = _loc2_;
         this.equippedSprite.alpha = _loc2_;
         this.equippedBorder.alpha = _loc2_;
      }
      
      override protected function updateData() : void
      {
         var _loc5_:uint = 0;
         var _loc6_:int = 0;
         var _loc7_:uint = 0;
         var _loc8_:ColorTransform = null;
         var _loc9_:Boolean = false;
         var _loc1_:ItemData = _data as ItemData;
         super.updateData();
         var _loc2_:Boolean = _loc1_.isStack;
         var _loc3_:Boolean = _loc1_.descriptor == 200 || _loc1_.descriptor == 300;
         var _loc4_:Boolean = !_loc2_ && !_loc3_;
         this.bundleBackground.visible = _loc1_.fromBundle;
         this.textName.text = _loc1_.name;
         if(Boolean(_loc1_.rank))
         {
            if(_loc1_.rank == 1)
            {
               _loc5_ = Colors.RANK_1;
            }
            else if(_data.rank == 2)
            {
               _loc5_ = Colors.RANK_2;
            }
            else
            {
               _loc5_ = Colors.RANK_3;
            }
         }
         else
         {
            _loc5_ = Colors.LIGHT_GRAY;
         }
         this.textName.textField.textColor = _loc5_;
         this.textLevel.text = _loc1_.itemLevel.toString();
         this.textLevel.visible = _loc1_.itemLevel > 0 && _loc1_.superCategory != Category.BASE_VOUCHER;
         this.sellerIcon.visible = Boolean(_loc1_.sellerIcon);
         if(Boolean(this.sellerIcon.numChildren))
         {
            this.sellerIcon.removeChildAt(0);
         }
         if(Boolean(_loc1_.sellerIcon))
         {
            this.sellerIcon.addChild(_loc1_.sellerIcon);
         }
         this.textLevel.x = Boolean(this.sellerIcon.numChildren) ? 24 : this.sellerIcon.x;
         trace("IRV::updateData",_loc1_.name,_loc1_.isStack,_loc1_.value,"shop:",_loc3_,this.equippedSprite,this.equippedBorder,"|",_loc1_.itemLevel,this.textLevel.text);
         if(_loc4_)
         {
            this.textFieldInfo.visible = MainScene.showDebugInfo;
            if(this.textFieldInfo.visible)
            {
               this.textFieldInfo.text = _loc1_.itemId.toString();
            }
            this.equippedSprite.visible = _loc1_.equipped;
            this.equippedBorder.visible = _loc1_.equipped;
            this.brokenIcon.visible = _loc1_.broken;
            if(this.brokenIcon.visible)
            {
               this.brokenIcon.x = this.equippedSprite.visible ? 145 : 160;
            }
            this.paintedIcon.visible = Boolean(_loc1_.upgrades) && _loc1_.upgrades.length > 0;
            this.updateScopeBadge(_loc1_,true);
            this.damageIndicator.visible = _loc4_ && _loc1_.value < 100;
            this.damageIndicator.track.scaleX = _data.value / 100;
            _loc6_ = _data.value / 100 * 510;
            _loc7_ = Util.getColor(Math.min(510 - _loc6_,255),Math.min(_loc6_,255),0);
            _loc8_ = new ColorTransform();
            _loc8_.color = _loc7_;
            this.damageIndicator.track.transform.colorTransform = _loc8_;
            this.damageIndicator.track.alpha = 0.4;
         }
         else
         {
            this.textFieldInfo.visible = false;
            this.equippedSprite.visible = false;
            this.equippedBorder.visible = false;
            this.brokenIcon.visible = false;
            this.paintedIcon.visible = false;
            this.updateScopeBadge(_loc1_,false);
            this.damageIndicator.visible = false;
         }
         if(_loc3_)
         {
            this.labelCost.visible = true;
            this.labelCost.moneyType = _loc1_.isPremium ? 1 : 0;
            this.labelCost.textField.textColor = _loc1_.isPremium ? Colors.GOLD : Colors.PRICE_REG;
            this.labelCost.cost = _loc1_.cost;
         }
         else
         {
            this.labelCost.visible = false;
         }
         if(_loc2_ && !_loc3_)
         {
            this.textFieldCount.visible = true;
            this.textFieldCount.text = "x" + _loc1_.value;
         }
         else
         {
            this.textFieldCount.visible = false;
         }
         if(Boolean(this.bottomBackground))
         {
            _loc9_ = _data.descriptor == 200 || _data.descriptor == 300;
            this.bottomBackground.height = _loc9_ ? 54 : 34;
            this.bottomBackground.y = _loc9_ ? 75 : 95;
         }
      }
   }
}

