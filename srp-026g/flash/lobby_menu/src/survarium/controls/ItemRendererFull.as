package survarium.controls
{
   import flash.display.MovieClip;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import survarium.Util;
   import survarium.constants.Colors;
   import survarium.data.ItemData;
   
   public class ItemRendererFull extends ItemRendererBase
   {
      
      public var textName:FixedText;
      
      public var textLevel:TextField;
      
      public var textFieldInfo:TextField;
      
      public var sellerIcon:MovieClip;
      
      public var damageIndicator:MovieClip;
      
      public var brokenIcon:MovieClip;
      
      public var paintedIcon:MovieClip;
      
      public var bundleBackground:MovieClip;
      
      public function ItemRendererFull()
      {
         super();
      }
      
      override public function updateState(param1:uint = 0) : void
      {
         var _loc2_:Number = param1 == 0 ? 1 : 0.6;
         if(iconRenderer)
         {
            iconRenderer.alpha = param1 == 2 ? 0.6 : 1;
         }
         this.textName.alpha = _loc2_;
         this.textLevel.alpha = _loc2_;
         this.textFieldInfo.alpha = _loc2_;
         this.sellerIcon.alpha = _loc2_;
         this.damageIndicator.alpha = _loc2_;
         this.brokenIcon.alpha = _loc2_;
         this.paintedIcon.alpha = _loc2_;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function draw() : void
      {
         super.draw();
      }
      
      override protected function updateData() : void
      {
         var _loc1_:ItemData = _data as ItemData;
         var _loc2_:uint;
         super.updateData();
         this.damageIndicator.visible = !_loc1_.isStack && _loc1_.value < 100;
         if(this.bundleBackground)
         {
            this.bundleBackground.visible = _loc1_.fromBundle;
         }
         this.textFieldInfo.visible = MainScene.showDebugInfo;
         if(this.textFieldInfo.visible)
         {
            this.textFieldInfo.text = _loc1_.itemId.toString();
         }
         trace("IRF::updateData",_loc1_.name,_loc1_.isStack,_loc1_.value);
         this.textName.text = _loc1_.name;
         if(_loc1_.rank)
         {
            if(_loc1_.rank == 1)
            {
               _loc2_ = Colors.RANK_1;
            }
            else if(_data.rank == 2)
            {
               _loc2_ = Colors.RANK_2;
            }
            else
            {
               _loc2_ = Colors.RANK_3;
            }
         }
         else
         {
            _loc2_ = Colors.LIGHT_GRAY;
         }
         this.textName.textField.textColor = _loc2_;
         this.textLevel.text = _loc1_.itemLevel.toString();
         this.textLevel.visible = _loc1_.itemLevel > 0;
         this.sellerIcon.visible = Boolean(_loc1_.sellerIcon);
         if(this.sellerIcon.numChildren)
         {
            this.sellerIcon.removeChildAt(0);
         }
         if(_loc1_.sellerIcon)
         {
            this.sellerIcon.addChild(_loc1_.sellerIcon);
         }
         this.textLevel.x = this.sellerIcon.numChildren ? 24 : this.sellerIcon.x;
         this.brokenIcon.visible = _loc1_.broken;
         this.paintedIcon.visible = _loc1_.upgrades && _loc1_.upgrades.length > 0;
         this.damageIndicator.track.scaleX = _data.value / 100;
         var _loc3_:int = _data.value / 100 * 510;
         var _loc4_:uint = Util.getColor(Math.min(510 - _loc3_,255),Math.min(_loc3_,255),0);
         var _loc5_:ColorTransform = new ColorTransform();
         _loc5_.color = _loc4_;
         this.damageIndicator.track.transform.colorTransform = _loc5_;
         this.damageIndicator.track.alpha = 0.4;
      }
   }
}

