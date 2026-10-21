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
      
      override public function updateState(state:uint = 0) : void
      {
         var contentAlpha:Number = state == 0 ? 1 : 0.6;
         if(iconRenderer)
         {
            iconRenderer.alpha = state == 2 ? 0.6 : 1;
         }
         this.textName.alpha = contentAlpha;
         this.textLevel.alpha = contentAlpha;
         this.textFieldInfo.alpha = contentAlpha;
         this.sellerIcon.alpha = contentAlpha;
         this.damageIndicator.alpha = contentAlpha;
         this.brokenIcon.alpha = contentAlpha;
         this.paintedIcon.alpha = contentAlpha;
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
         var item:ItemData = _data as ItemData;
         var nameColor:uint;
         super.updateData();
         this.damageIndicator.visible = !item.isStack && item.value < 100;
         if(this.bundleBackground)
         {
            this.bundleBackground.visible = item.fromBundle;
         }
         this.textFieldInfo.visible = MainScene.showDebugInfo;
         if(this.textFieldInfo.visible)
         {
            this.textFieldInfo.text = item.itemId.toString();
         }
         trace("IRF::updateData",item.name,item.isStack,item.value);
         this.textName.text = item.name;
         if(item.rank)
         {
            if(item.rank == 1)
            {
               nameColor = Colors.RANK_1;
            }
            else if(_data.rank == 2)
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
         this.textName.textField.textColor = nameColor;
         this.textLevel.text = item.itemLevel.toString();
         this.textLevel.visible = item.itemLevel > 0;
         this.sellerIcon.visible = Boolean(item.sellerIcon);
         if(this.sellerIcon.numChildren)
         {
            this.sellerIcon.removeChildAt(0);
         }
         if(item.sellerIcon)
         {
            this.sellerIcon.addChild(item.sellerIcon);
         }
         this.textLevel.x = this.sellerIcon.numChildren ? 24 : this.sellerIcon.x;
         this.brokenIcon.visible = item.broken;
         this.paintedIcon.visible = item.upgrades && item.upgrades.length > 0;
         this.damageIndicator.track.scaleX = _data.value / 100;
         var condition:int = _data.value / 100 * 510;
         var trackColor:uint = Util.getColor(Math.min(510 - condition,255),Math.min(condition,255),0);
         var colorTransform:ColorTransform = new ColorTransform();
         colorTransform.color = trackColor;
         this.damageIndicator.track.transform.colorTransform = colorTransform;
         this.damageIndicator.track.alpha = 0.4;
      }
   }
}

